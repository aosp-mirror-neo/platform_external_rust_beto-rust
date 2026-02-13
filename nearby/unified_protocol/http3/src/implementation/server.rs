// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
//
// you may not use this file except in compliance with the License.
//
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
//
// distributed under the License is distributed on an "AS IS" BASIS,
//
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
// See the License for the specific language governing permissions and
//
// limitations under the License.
//
//

use super::connection::{ConnectionState, QuicheHttp3Connection};
use super::test_certs::{TEST_SERVER_CERT, TEST_SERVER_KEY};
use super::{
    BUFFER_SIZE, MAX_CONCURRENT_STREAM_SIZE, QUICHE_IDLE_TIMEOUT_MS,
    QUICHE_MAX_INCOMING_BUFFER_SIZE, QUICHE_UDP_PAYLOAD_SIZE,
};
use crate::{
    ConnectionID, Http3ConnectionEvent, Http3Error, Http3Server, HttpResponse, PacketReceiver,
    PacketSender, PeerInfo, MAX_CONTROL_MESSAGE_SIZE,
};
use async_trait::async_trait;
use log::{debug, error, info, trace, warn};
use std::collections::{hash_map, HashMap};
use std::future;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;

// Per RFC 9000 section 7.2, an Initial packet's dcid from a new client must be
// at least 8 bytes in length. We use the first 8 bytes of dcid as new connection
// ID to identify the client.
// This is helpful to identify 0-RTT packets. In 0-RTT handshake, 0-RTT packets
// are followed after the Initial packet with the same dcid. With this function, we
// know which 0-RTT packets belong to which client.
const CONN_ID_LEN: usize = 8;
const CHANNEL_SIZE: usize = 100;

fn get_conn_id(hdr: &quiche::Header) -> Result<ConnectionID, Http3Error> {
    if let Some(v) = hdr.dcid.as_ref().get(0..CONN_ID_LEN) {
        return Ok(v.to_vec());
    }
    Err(Http3Error::InvalidConnectionID)
}

fn create_quiche_config() -> Result<quiche::Config, Http3Error> {
    let mut quiche_config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

    // Use tempfile to write the certificate and the private key to files for Quiche to read.
    let mut cert_file = tempfile::Builder::new().suffix(".crt").tempfile()?;
    cert_file.write_all(TEST_SERVER_CERT.as_bytes())?;
    let cert_path = cert_file
        .path()
        .to_str()
        .ok_or(Http3Error::InternalError("Failed to get certificate file path".to_string()))?;
    quiche_config.load_cert_chain_from_pem_file(cert_path)?;

    let mut key_file = tempfile::Builder::new().suffix(".key").tempfile()?;
    key_file.write_all(TEST_SERVER_KEY.as_bytes())?;
    let key_path = key_file
        .path()
        .to_str()
        .ok_or(Http3Error::InternalError("Failed to get private key file path".to_string()))?;
    quiche_config.load_priv_key_from_pem_file(key_path)?;

    quiche_config.set_application_protos(quiche::h3::APPLICATION_PROTOCOL)?;
    quiche_config.set_max_idle_timeout(QUICHE_IDLE_TIMEOUT_MS);
    quiche_config.set_max_recv_udp_payload_size(QUICHE_UDP_PAYLOAD_SIZE);
    quiche_config.set_max_send_udp_payload_size(QUICHE_UDP_PAYLOAD_SIZE);
    quiche_config.set_initial_max_data(QUICHE_MAX_INCOMING_BUFFER_SIZE);
    quiche_config.set_initial_max_stream_data_bidi_local(QUICHE_MAX_INCOMING_BUFFER_SIZE);
    quiche_config.set_initial_max_stream_data_bidi_remote(QUICHE_MAX_INCOMING_BUFFER_SIZE);
    quiche_config.set_initial_max_stream_data_uni(QUICHE_MAX_INCOMING_BUFFER_SIZE);
    quiche_config.set_initial_max_streams_bidi(MAX_CONCURRENT_STREAM_SIZE);
    quiche_config.set_initial_max_streams_uni(MAX_CONCURRENT_STREAM_SIZE);
    quiche_config.set_disable_active_migration(true);
    quiche_config.enable_early_data();

    Ok(quiche_config)
}

struct ConnectionManager {
    clients: HashMap<ConnectionID, QuicheHttp3Connection>,
    config: quiche::Config,
}

impl ConnectionManager {
    fn new(config: quiche::Config) -> Result<ConnectionManager, Http3Error> {
        Ok(ConnectionManager { clients: HashMap::new(), config })
    }

    fn get_or_create(
        &mut self,
        hdr: &quiche::Header,
        peer: SocketAddr,
        local: SocketAddr,
        h3_event_sender: &mpsc::Sender<Http3ConnectionEvent>,
    ) -> Result<&mut QuicheHttp3Connection, Http3Error> {
        let conn_id = get_conn_id(hdr)?;
        let client = match self.clients.entry(conn_id.clone()) {
            hash_map::Entry::Occupied(client) => client.into_mut(),
            hash_map::Entry::Vacant(vacant) => {
                if hdr.ty != quiche::Type::Initial {
                    debug!("Packet is not Initial");
                    return Err(Http3Error::InvalidConnectionID);
                }
                if !quiche::version_is_supported(hdr.version) {
                    debug!("Protocol version not supported");
                    return Err(Http3Error::InvalidConnectionID);
                }
                let conn = quiche::accept(
                    &quiche::ConnectionId::from_ref(&conn_id),
                    None, /* odcid */
                    local,
                    peer,
                    &mut self.config,
                )?;
                let (state_tx, _state_rx) = watch::channel(ConnectionState::Connecting);
                let client = QuicheHttp3Connection::new(
                    conn,
                    local,
                    peer,
                    conn_id.clone(),
                    h3_event_sender.clone(),
                    true,
                    state_tx,
                );
                info!("New client: {client:?}");
                vacant.insert(client)
            }
        };
        Ok(client)
    }

    fn get_mut(&mut self, id: &[u8]) -> Option<&mut QuicheHttp3Connection> {
        self.clients.get_mut(id)
    }

    fn iter_mut(&mut self) -> hash_map::IterMut<ConnectionID, QuicheHttp3Connection> {
        self.clients.iter_mut()
    }

    fn values(&self) -> hash_map::Values<ConnectionID, QuicheHttp3Connection> {
        self.clients.values()
    }
}

pub(crate) struct QuicheHttp3Server {
    task_handle: JoinHandle<()>,
    h3_event_receiver: Arc<Mutex<mpsc::Receiver<Http3ConnectionEvent>>>,
    internal_cmd_tx: mpsc::Sender<Command>,
}

struct ServerWorker {
    packet_sender: Box<dyn PacketSender>,
    packet_receiver: Box<dyn PacketReceiver>,
    local_addr: SocketAddr,
    clients: ConnectionManager,
    internal_cmd_rx: mpsc::Receiver<Command>,
    buffer: Vec<u8>,
    h3_event_sender: mpsc::Sender<Http3ConnectionEvent>,
}

impl QuicheHttp3Server {
    pub(crate) fn new(
        packet_sender: Box<dyn PacketSender>,
        packet_receiver: Box<dyn PacketReceiver>,
        local_addr: SocketAddr,
    ) -> Result<Self, Http3Error> {
        let (h3_event_sender, h3_event_receiver) = mpsc::channel(CHANNEL_SIZE);
        let (internal_cmd_tx, internal_cmd_rx) = mpsc::channel(CHANNEL_SIZE);

        let mut worker = ServerWorker::new(
            packet_sender,
            packet_receiver,
            local_addr,
            h3_event_sender,
            internal_cmd_rx,
        )?;

        let task_handle = tokio::spawn(async move {
            worker.run().await;
        });

        Ok(Self {
            task_handle,
            h3_event_receiver: Arc::new(Mutex::new(h3_event_receiver)),
            internal_cmd_tx,
        })
    }
}
/// Internal commands used by the HTTP/3 server worker.
#[derive(Debug)]
enum Command {
    /// Send an HTTP response.
    SendHttpResponse { peer_info: PeerInfo, stream_id: u64, response: HttpResponse },
    /// Send a control message.
    SendControlMessage { peer_info: PeerInfo, message: Vec<u8> },
    /// Close all connections.
    CloseAll { reason: Option<Vec<u8>> },
}

impl ServerWorker {
    fn new(
        packet_sender: Box<dyn PacketSender>,
        packet_receiver: Box<dyn PacketReceiver>,
        local_addr: SocketAddr,
        h3_event_sender: mpsc::Sender<Http3ConnectionEvent>,
        internal_cmd_rx: mpsc::Receiver<Command>,
    ) -> Result<Self, Http3Error> {
        let clients = ConnectionManager::new(create_quiche_config()?)?;

        Ok(Self {
            packet_sender,
            packet_receiver,
            local_addr,
            clients,
            internal_cmd_rx,
            buffer: vec![0; BUFFER_SIZE],
            h3_event_sender,
        })
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                // Selects the earliest timeout among all clients.
                _ = async {
                    let min_peer_timeout = self.clients.values().filter_map(|c| c.timeout()).min();
                    if let Some(timeout) = min_peer_timeout {
                        tokio::time::sleep(timeout).await
                    } else {
                        future::pending().await
                    }
                } => {
                    self.handle_timeout().await;
                }
                res = self.packet_receiver.recv_from(&mut self.buffer) => {
                    match res {
                        Ok((len, peer)) => {
                            self.handle_socket_input(len, peer).await;
                        }
                        Err(e) => {
                            error!("Packet receiver error: {e:?}");
                            // Avoid busy loop on error
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }
                Some(command) = self.internal_cmd_rx.recv() => {
                    self.handle_internal_command(command).await;
                }
            }
        }
    }

    async fn flush_peer_egress(&mut self, connection_id: &ConnectionID) {
        if let Some(client) = self.clients.get_mut(connection_id) {
            while let Ok(v) = client.flush_egress() {
                let addr = client.remote_addr();
                trace!("Sending {} bytes to client {}", v.len(), addr);
                if let Err(e) = self.packet_sender.send_to(&v, addr).await {
                    error!("Failed to send packet to {client:?}: {e:?}");
                }
            }
        }
    }

    async fn handle_timeout(&mut self) {
        let mut connection_ids = Vec::new();
        let mut closed_connections = Vec::new();
        for (conn_id, client) in self.clients.iter_mut() {
            client.on_timeout().await;
            if client.is_closed() {
                closed_connections.push(conn_id.clone());
            } else {
                connection_ids.push(conn_id.clone());
            }
        }

        for conn_id in closed_connections {
            debug!("Removing closed connection: {conn_id:?}");
            let _ = self.clients.clients.remove(&conn_id);
        }
        for connection_id in connection_ids {
            self.flush_peer_egress(&connection_id).await;
        }
    }

    async fn handle_socket_input(&mut self, len: usize, peer: SocketAddr) {
        info!("Got {len} bytes from {peer}");
        let local = self.local_addr;

        let Some(pkt_buf) = self.buffer.get_mut(..len) else {
            error!("Buffer size less than data size {len}");
            return;
        };
        let hdr = match quiche::Header::from_slice(pkt_buf, CONN_ID_LEN) {
            Ok(v) => v,
            Err(e) => {
                debug!("Failed to parse QUIC header: {e:?}");
                return;
            }
        };

        trace!("Got QUIC packet: {hdr:?}");

        let client = match self.clients.get_or_create(&hdr, peer, local, &self.h3_event_sender) {
            Ok(v) => v,
            Err(e) => {
                debug!("Failed to get client by hdr {hdr:?}: {e}");
                return;
            }
        };

        match client.process_packet(pkt_buf).await {
            Ok(_) => {
                // handle_http3_request now sends events internally
            }
            Err(e) => {
                debug!("Failed to process QUIC packet: {e}");
                return;
            }
        }

        if let Err(e) = client.resume_writes() {
            warn!("Failed to resume writes: {e:?}");
        }

        let connection_id = client.connection_id().clone();
        self.flush_peer_egress(&connection_id).await;
    }

    async fn handle_internal_command(&mut self, command: Command) {
        trace!("handle_internal_command: {command:?}");
        match command {
            Command::SendHttpResponse { peer_info, stream_id, response } => {
                debug!("Got SendHttpResponse for stream {stream_id}: {response:?}");
                let connection_id = peer_info.connection_id.clone();
                if let Some(client) = self.clients.get_mut(&connection_id) {
                    let _ = client.send_http_response(stream_id, response).await;
                    self.flush_peer_egress(&connection_id).await;
                } else {
                    warn!("No client found for connection_id {connection_id:?}");
                }
            }
            Command::SendControlMessage { peer_info, message } => {
                let connection_id = peer_info.connection_id.clone();
                if let Some(client) = self.clients.get_mut(&connection_id) {
                    if let Err(e) = client.send_control_message(&message) {
                        error!("Failed to send control message: {e:?}");
                    }
                    self.flush_peer_egress(&connection_id).await;
                } else {
                    warn!("No client found for connection_id {connection_id:?}");
                }
            }
            Command::CloseAll { reason } => {
                info!(
                    "Closing all connections: reason={:?}",
                    reason.as_deref().map(String::from_utf8_lossy)
                );
                let reason_bytes = reason.as_deref().unwrap_or(b"");
                let mut connection_ids = Vec::new();
                for (conn_id, client) in self.clients.iter_mut() {
                    if let Err(e) = client.close(true, 0, reason_bytes) {
                        warn!("Failed to close connection {conn_id:?}: {e:?}");
                    }
                    connection_ids.push(conn_id.clone());
                }

                for connection_id in connection_ids {
                    self.flush_peer_egress(&connection_id).await;
                }
            }
        }
    }
}

impl Drop for QuicheHttp3Server {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

#[async_trait]
impl Http3Server for QuicheHttp3Server {
    async fn event(&self) -> Option<Http3ConnectionEvent> {
        let mut receiver = self.h3_event_receiver.lock().await;
        receiver.recv().await
    }
    async fn send_response(
        &self,
        peer_info: crate::PeerInfo,
        stream_id: u64,
        response: HttpResponse,
    ) -> Result<(), Http3Error> {
        self.internal_cmd_tx
            .send(Command::SendHttpResponse { peer_info, stream_id, response })
            .await
            .map_err(|e| {
                Http3Error::InternalError(format!("Failed to send response command: {e}"))
            })?;
        Ok(())
    }

    async fn send_control_message(
        &self,
        peer_info: PeerInfo,
        message: &[u8],
    ) -> Result<(), Http3Error> {
        if message.len() > MAX_CONTROL_MESSAGE_SIZE {
            return Err(Http3Error::InvalidRequest(format!(
                "Control message size {} exceeds maximum allowed size {}",
                message.len(),
                MAX_CONTROL_MESSAGE_SIZE
            )));
        }

        self.internal_cmd_tx
            .send(Command::SendControlMessage { peer_info, message: message.to_vec() })
            .await
            .map_err(|e| {
                Http3Error::InternalError(format!("Failed to send SendControlMessage command: {e}"))
            })?;
        Ok(())
    }

    async fn close(&self, reason: Option<&[u8]>) -> Result<(), Http3Error> {
        self.internal_cmd_tx
            .send(Command::CloseAll { reason: reason.map(|r| r.to_vec()) })
            .await
            .map_err(|e| Http3Error::InternalError(format!("Failed to send CloseAll command: {e}")))
    }
}
