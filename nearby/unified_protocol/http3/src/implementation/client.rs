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
use super::test_certs::TEST_SERVER_NAME;
use super::{
    BUFFER_SIZE, MAX_CONCURRENT_STREAM_SIZE, QUICHE_IDLE_TIMEOUT_MS,
    QUICHE_MAX_INCOMING_BUFFER_SIZE, QUICHE_UDP_PAYLOAD_SIZE,
};
use crate::{
    Http3Client, Http3ConnectionEvent, Http3Error, HttpRequest, PacketReceiver, PacketSender,
    MAX_CONTROL_MESSAGE_SIZE,
};
use async_trait::async_trait;
use log::{debug, error, trace, warn};
use rand::Rng;
use std::future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio::task::JoinHandle;

const CID_LEN: usize = 16;
const CHANNEL_SIZE: usize = 100;

fn new_scid() -> [u8; CID_LEN] {
    rand::thread_rng().gen()
}

fn create_quiche_config() -> Result<quiche::Config, Http3Error> {
    let mut quiche_config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

    quiche_config.verify_peer(false);

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

    Ok(quiche_config)
}

pub(crate) struct QuicheHttp3Client {
    task_handle: JoinHandle<()>,
    h3_event_receiver: Arc<Mutex<mpsc::Receiver<Http3ConnectionEvent>>>,
    internal_cmd_tx: mpsc::Sender<Command>,
}

struct ClientWorker {
    packet_sender: Box<dyn PacketSender>,
    packet_receiver: Box<dyn PacketReceiver>,
    peer_addr: SocketAddr,
    peer: QuicheHttp3Connection,
    internal_cmd_rx: mpsc::Receiver<Command>,
    buffer: Vec<u8>,
}

impl QuicheHttp3Client {
    pub(crate) fn new(
        packet_sender: Box<dyn PacketSender>,
        packet_receiver: Box<dyn PacketReceiver>,
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
    ) -> Result<Self, Http3Error> {
        let (h3_event_sender, h3_event_receiver) = mpsc::channel(CHANNEL_SIZE);
        let (internal_cmd_tx, internal_cmd_rx) = mpsc::channel(CHANNEL_SIZE);

        let mut worker = ClientWorker::new(
            packet_sender,
            packet_receiver,
            local_addr,
            peer_addr,
            h3_event_sender,
            internal_cmd_rx,
            internal_cmd_tx.clone(),
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

/// Internal commands used by the HTTP/3 client worker.
enum Command {
    /// Send an HTTP request.
    SendHttpRequest { request: HttpRequest, tx: oneshot::Sender<u64> },
    /// Close the connection.
    Close { reason: Option<Vec<u8>> },
    /// Send a control message.
    SendControlMessage { message: Vec<u8> },
    /// Create a bidirectional stream.
    CreateBidiStream { stream_type: CreateBidiStreamType },
}

enum CreateBidiStreamType {
    ControlStream,
}

fn spawn_control_stream_creator(
    mut connected_rx: watch::Receiver<ConnectionState>,
    cmd_tx: mpsc::Sender<Command>,
) {
    #[allow(clippy::let_underscore_future)]
    let _ = tokio::spawn(async move {
        if connected_rx.wait_for(|&state| state == ConnectionState::Connected).await.is_err() {
            // The sender was dropped because the connection handshake failed.
            return;
        }

        if let Err(e) = cmd_tx
            .send(Command::CreateBidiStream { stream_type: CreateBidiStreamType::ControlStream })
            .await
        {
            warn!("Failed to send CreateBidiStream command: {e:?}");
        }
    });
}

impl ClientWorker {
    fn new(
        packet_sender: Box<dyn PacketSender>,
        packet_receiver: Box<dyn PacketReceiver>,
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
        h3_event_sender: mpsc::Sender<Http3ConnectionEvent>,
        internal_cmd_rx: mpsc::Receiver<Command>,
        internal_cmd_tx: mpsc::Sender<Command>,
    ) -> Result<Self, Http3Error> {
        let mut config = create_quiche_config()?;
        let scid = new_scid();
        debug!("ClientWorker: {local_addr:?}, {peer_addr:?}");

        let conn = quiche::connect(
            Some(TEST_SERVER_NAME),
            &quiche::ConnectionId::from_ref(&scid),
            local_addr,
            peer_addr,
            &mut config,
        )?;
        let (state_tx, state_rx) = watch::channel(ConnectionState::Connecting);

        let peer = QuicheHttp3Connection::new(
            conn,
            local_addr,
            peer_addr,
            scid.to_vec(),
            h3_event_sender.clone(),
            false,
            state_tx,
        );

        spawn_control_stream_creator(state_rx, internal_cmd_tx.clone());

        Ok(Self {
            packet_sender,
            packet_receiver,
            peer_addr,
            peer,
            internal_cmd_rx,
            buffer: vec![0; BUFFER_SIZE],
        })
    }

    async fn run(&mut self) {
        // Initiate the handshake.
        self.flush_peer_egress().await;

        loop {
            tokio::select! {
                _ = async {
                    if let Some(timeout) = self.peer.timeout() {
                        tokio::time::sleep(timeout).await
                    } else {
                        future::pending().await
                    }
                } => {
                    self.handle_timeout().await;
                }
                Ok((len, from)) = self.packet_receiver.recv_from(&mut self.buffer) => {
                    if from == self.peer_addr {
                        self.handle_socket_input(len).await;
                    } else {
                        trace!("Received packet from unknown peer: {from}");
                    }
                }
                Some(command) = self.internal_cmd_rx.recv() => {
                    self.handle_internal_command(command).await;
                }
            }
        }
    }

    async fn flush_peer_egress(&mut self) {
        while let Ok(v) = self.peer.flush_egress() {
            if let Err(e) = self.packet_sender.send_to(&v, self.peer_addr).await {
                error!("Failed to send packet: {e:?}");
            }
        }
    }

    async fn handle_timeout(&mut self) {
        log::info!("Client timeout");
        self.peer.on_timeout().await;
        self.flush_peer_egress().await;
    }

    async fn handle_socket_input(&mut self, len: usize) {
        trace!("Got {len} bytes from peer");
        let Some(pkt_buf) = self.buffer.get_mut(..len) else {
            error!("Buffer size less than data size {len}");
            return;
        };
        match self.peer.process_packet(pkt_buf).await {
            Ok(_) => {
                // handle_http3_request now sends events internally
            }
            Err(e) => {
                debug!("Failed to process QUIC packet: {e}");
                return;
            }
        }

        if let Err(e) = self.peer.resume_writes() {
            warn!("Failed to resume writes: {e:?}");
        }

        self.flush_peer_egress().await;
    }

    async fn handle_internal_command(&mut self, command: Command) {
        match command {
            Command::SendHttpRequest { request, tx } => {
                debug!("Got HttpRequest: {request:?}");
                let result = self.peer.send_http_request(request).await;
                self.flush_peer_egress().await;
                match result {
                    Ok(stream_id) => {
                        if tx.send(stream_id).is_err() {
                            warn!("Failed to send stream ID back to caller");
                        }
                    }
                    Err(e) => {
                        error!("Failed to send HTTP request: {e:?}");
                    }
                }
            }
            Command::Close { reason } => {
                let reason_bytes = reason.as_deref().unwrap_or(b"");
                log::info!(
                    "Closing connection: reason={:?}",
                    String::from_utf8_lossy(reason_bytes)
                );
                if let Err(e) = self.peer.close(true, 0, reason_bytes) {
                    warn!("Failed to close connection: {e:?}");
                }
                self.flush_peer_egress().await;
            }
            Command::CreateBidiStream { stream_type } => match stream_type {
                CreateBidiStreamType::ControlStream => {
                    let request = HttpRequest {
                        method: crate::Method::Connect,
                        uri: super::connection::CONTROL_STREAM_URI.to_string(),
                        headers: crate::Headers::new(),
                        body: None,
                    };
                    match self.peer.send_http_request(request).await {
                        Ok(stream_id) => {
                            debug!("Control stream created: {stream_id}");
                            self.peer.set_control_stream_id(stream_id);
                        }
                        Err(e) => {
                            error!("Failed to create control stream: {e:?}");
                        }
                    }
                    self.flush_peer_egress().await;
                }
            },
            Command::SendControlMessage { message } => {
                if let Err(e) = self.peer.send_control_message(&message) {
                    error!("Failed to send control message: {e:?}");
                }
                self.flush_peer_egress().await;
            }
        }
    }
}

impl Drop for QuicheHttp3Client {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

#[async_trait]
impl Http3Client for QuicheHttp3Client {
    async fn event(&self) -> Option<Http3ConnectionEvent> {
        let mut receiver = self.h3_event_receiver.lock().await;
        receiver.recv().await
    }
    async fn request(&self, request: HttpRequest) -> Result<u64, Http3Error> {
        if request.method == crate::Method::Connect {
            return Err(Http3Error::InvalidRequest(
                "Method CONNECT is for internal use only".to_string(),
            ));
        }
        let (tx, rx) = oneshot::channel();
        self.internal_cmd_tx.send(Command::SendHttpRequest { request, tx }).await.map_err(|e| {
            error!("Failed to send HttpRequest: {e:?}");
            Http3Error::InternalError(format!("Failed to send HttpRequest: {e}"))
        })?;
        rx.await.map_err(|e| {
            error!("Failed to receive stream ID: {e:?}");

            Http3Error::InternalError(format!("Failed to receive stream ID: {e}"))
        })
    }

    async fn send_control_message(&self, message: &[u8]) -> Result<(), Http3Error> {
        if message.len() > MAX_CONTROL_MESSAGE_SIZE {
            return Err(Http3Error::InvalidRequest(format!(
                "Control message size {} exceeds maximum allowed size {}",
                message.len(),
                MAX_CONTROL_MESSAGE_SIZE
            )));
        }

        self.internal_cmd_tx
            .send(Command::SendControlMessage { message: message.to_vec() })
            .await
            .map_err(|e| {
                error!("Failed to send SendControlMessage command: {e:?}");
                Http3Error::InternalError(format!("Failed to send SendControlMessage command: {e}"))
            })
    }

    async fn close(&self, reason: Option<&[u8]>) -> Result<(), Http3Error> {
        self.internal_cmd_tx
            .send(Command::Close { reason: reason.map(|r| r.to_vec()) })
            .await
            .map_err(|e| {
                error!("Failed to send Close command: {e:?}");
                Http3Error::QuicheError(quiche::Error::Done)
            })
    }
}
