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

use super::{BUFFER_SIZE, FILE_STREAM_SIZE};
use crate::{
    BodyReader, ConnectionID, Content, Headers, Http3ConnectionEvent, Http3Error, HttpRequest,
    Method,
};
use log::{debug, error, trace, warn};
use quiche::h3::NameValue;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

// Named constants for header names
const HEADER_METHOD: &[u8] = b":method";
const HEADER_PATH: &[u8] = b":path";
const HEADER_SCHEME: &[u8] = b":scheme";
const HEADER_STATUS: &[u8] = b":status";

pub(crate) const CONTROL_STREAM_URI: &str = "/up_http3_ctrl_channel";

#[derive(Debug)]
struct OutgoingDataChunk {
    data: Vec<u8>,
    offset: usize,
    fin: bool,
}

#[derive(Debug)]
struct OutgoingStreamState {
    queue: VecDeque<OutgoingDataChunk>,
}

#[derive(Debug)]
enum StreamState {
    HeaderReceived { headers: Vec<quiche::h3::Header> },
    ReceivingBody { writer: tokio::io::DuplexStream },
    ControlStreamConnecting,
    ControlStreamConnected { buffer: Vec<u8> },
}

#[derive(Default)]
struct StreamManager {
    streams: HashMap<u64, StreamState>,
}

impl StreamManager {
    fn insert(&mut self, stream_id: u64, state: StreamState) {
        let _ = self.streams.insert(stream_id, state);
    }

    fn remove(&mut self, stream_id: u64) -> Option<StreamState> {
        self.streams.remove(&stream_id)
    }

    fn get_mut(&mut self, stream_id: u64) -> Option<&mut StreamState> {
        self.streams.get_mut(&stream_id)
    }
}

struct ConnectionInner {
    remote_addr: SocketAddr,
    local_addr: SocketAddr,
    id: ConnectionID,
    h3_event_sender: mpsc::Sender<Http3ConnectionEvent>,
    stream_manager: StreamManager,
    outgoing_streams: HashMap<u64, OutgoingStreamState>,
    is_server: bool,
    control_stream_id: Option<u64>,
}

impl ConnectionInner {
    fn new(
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        id: ConnectionID,
        h3_event_sender: mpsc::Sender<Http3ConnectionEvent>,
        is_server: bool,
    ) -> Self {
        Self {
            local_addr,
            remote_addr,
            id,
            h3_event_sender,
            stream_manager: StreamManager::default(),
            outgoing_streams: HashMap::new(),
            is_server,
            control_stream_id: None,
        }
    }

    async fn handle_request_headers(
        &mut self,
        stream_id: u64,
        list: Vec<quiche::h3::Header>,
        has_body: bool,
        h3_conn: &mut quiche::h3::Connection,
        quiche_conn: &mut quiche::Connection,
    ) -> Result<(), Http3Error> {
        let (method, uri, headers) = Self::parse_request_headers(&list);

        if method == Method::Connect {
            return self.handle_request_headers_for_connect(stream_id, &uri, h3_conn, quiche_conn);
        }

        self.stream_manager.insert(stream_id, StreamState::HeaderReceived { headers: list });

        let (reader, writer) = if has_body {
            let (r, w) = tokio::io::duplex(FILE_STREAM_SIZE);
            (Some(BodyReader(Box::pin(r))), Some(w))
        } else {
            (None, None)
        };

        if let Some(w) = writer {
            self.stream_manager.insert(stream_id, StreamState::ReceivingBody { writer: w });
        }

        let peer_info = crate::PeerInfo { connection_id: self.id.clone(), addr: self.remote_addr };

        let event = Http3ConnectionEvent::OnRequestReceived {
            peer_info,
            stream_id,
            method,
            uri,
            headers,
            body_reader: reader,
        };

        self.h3_event_sender.send(event).await?;
        Ok(())
    }

    fn handle_request_headers_for_connect(
        &mut self,
        stream_id: u64,
        uri: &str,
        h3_conn: &mut quiche::h3::Connection,
        quiche_conn: &mut quiche::Connection,
    ) -> Result<(), Http3Error> {
        match uri {
            CONTROL_STREAM_URI => {
                self.control_stream_id = Some(stream_id);
                self.stream_manager
                    .insert(stream_id, StreamState::ControlStreamConnected { buffer: Vec::new() });
                let response_headers = vec![quiche::h3::Header::new(b":status", b"200")];
                h3_conn.send_response(quiche_conn, stream_id, &response_headers, false)?;
                Ok(())
            }
            _ => Err(Http3Error::InvalidRequest(format!("Invalid CONNECT URI: {uri}"))),
        }
    }

    async fn handle_response_headers(
        &mut self,
        stream_id: u64,
        list: Vec<quiche::h3::Header>,
        has_body: bool,
    ) -> Result<(), Http3Error> {
        if let Some(ctrl_id) = self.control_stream_id {
            if ctrl_id == stream_id {
                // TODO: Check the status code. If the server rejected the control
                // stream, the client should handle it.
                self.stream_manager
                    .insert(stream_id, StreamState::ControlStreamConnected { buffer: Vec::new() });
                return Ok(());
            }
        }

        let (status, headers) = Self::parse_response_headers(&list);

        self.stream_manager
            .insert(stream_id, StreamState::HeaderReceived { headers: list.clone() });

        let (reader, writer) = if has_body {
            let (r, w) = tokio::io::duplex(FILE_STREAM_SIZE);
            (Some(BodyReader(Box::pin(r))), Some(w))
        } else {
            (None, None)
        };

        if let Some(w) = writer {
            self.stream_manager.insert(stream_id, StreamState::ReceivingBody { writer: w });
        }

        let peer_info = crate::PeerInfo { connection_id: self.id.clone(), addr: self.remote_addr };

        let event = Http3ConnectionEvent::OnResponseReceived {
            peer_info,
            stream_id,
            status,
            headers,
            body_reader: reader,
        };

        self.h3_event_sender.send(event).await?;
        Ok(())
    }

    async fn handle_http_body(
        &mut self,
        stream_id: u64,
        h3_conn: &mut quiche::h3::Connection,
        quiche_conn: &mut quiche::Connection,
    ) -> Result<(), Http3Error> {
        let state = self.stream_manager.get_mut(stream_id);
        match state {
            Some(StreamState::ControlStreamConnected { buffer }) => {
                let mut buf = [0; BUFFER_SIZE];
                loop {
                    match h3_conn.recv_body(quiche_conn, stream_id, &mut buf) {
                        Ok(read) => {
                            trace!("[{stream_id}] Received DATA {read} bytes");
                            if read == 0 {
                                break;
                            }
                            if let Some(data) = buf.get(..read) {
                                buffer.extend_from_slice(data);
                            }

                            loop {
                                if buffer.len() < 4 {
                                    break;
                                }

                                let len_bytes: [u8; 4] =
                                    match buffer.get(0..4).and_then(|s| s.try_into().ok()) {
                                        Some(b) => b,
                                        None => break,
                                    };
                                let len = u32::from_be_bytes(len_bytes) as usize;

                                if buffer.len() < 4 + len {
                                    break;
                                }

                                let message = match buffer.get(4..4 + len) {
                                    Some(m) => m.to_vec(),
                                    None => break,
                                };
                                drop(buffer.drain(0..4 + len));

                                let peer_info = crate::PeerInfo {
                                    connection_id: self.id.clone(),
                                    addr: self.remote_addr,
                                };
                                self.h3_event_sender
                                    .send(Http3ConnectionEvent::OnControlMessageReceived {
                                        peer_info,
                                        message,
                                    })
                                    .await?;
                            }
                        }
                        Err(quiche::h3::Error::Done) => break,
                        Err(e) => return Err(e.into()),
                    }
                }
            }
            Some(StreamState::ReceivingBody { writer }) => {
                let mut buf = [0; BUFFER_SIZE];
                loop {
                    match h3_conn.recv_body(quiche_conn, stream_id, &mut buf) {
                        Ok(read) => {
                            trace!("[{stream_id}] Received DATA {read} bytes");
                            if read == 0 {
                                break;
                            }
                            if let Some(data) = buf.get(..read) {
                                // TODO: This blocks the event loop if the consumer is
                                // slow. Consider stopping reading from quiche for this stream until
                                // the writer is ready.
                                if let Err(e) = writer.write_all(data).await {
                                    warn!("[{stream_id}] Failed to write to stream body pipe: {e}");
                                    // Continue consuming data to drain quiche buffer
                                }
                            }
                        }
                        Err(quiche::h3::Error::Done) => break,
                        Err(e) => return Err(e.into()),
                    }
                }
            }
            Some(StreamState::HeaderReceived { headers }) => {
                error!("[{stream_id}] Received data in HeaderReceived state: {headers:?}");
            }
            Some(StreamState::ControlStreamConnecting) => {
                warn!("[{stream_id}] Received data in ControlStreamConnecting state");
            }
            None => {
                debug!("[{stream_id}] Received data for unknown stream");
            }
        }
        Ok(())
    }

    fn handle_finished(&mut self, stream_id: u64) {
        debug!("[{stream_id}] STREAM FINISHED");
        let _ = self.stream_manager.remove(stream_id);
    }

    fn parse_request_headers(headers: &[quiche::h3::Header]) -> (Method, String, Headers) {
        let mut method = Method::Get;
        let mut path = String::new();
        let mut http_headers = Headers::new();

        for header in headers {
            let name = String::from_utf8_lossy(header.name());
            let value = String::from_utf8_lossy(header.value());

            http_headers.append(&name, &value);

            match name.as_ref() {
                ":method" => {
                    method = match value.as_ref() {
                        "POST" => Method::Post,
                        "CONNECT" => Method::Connect,
                        _ => Method::Get,
                    }
                }
                ":path" => {
                    path = value.to_string();
                }
                _ => {}
            }
        }
        (method, path, http_headers)
    }

    fn parse_response_headers(headers: &[quiche::h3::Header]) -> (u16, Headers) {
        let mut status = 0u16;
        let mut http_headers = Headers::new();

        for header in headers {
            let name = String::from_utf8_lossy(header.name());
            let value = String::from_utf8_lossy(header.value());
            match name.as_ref() {
                ":status" => status = value.parse().unwrap_or(0),
                _ => http_headers.append(&name, &value),
            }
        }
        (status, http_headers)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionState {
    Connecting,
    Connected,
}

pub(crate) struct QuicheHttp3Connection {
    quiche_conn: quiche::Connection,
    h3_conn: Option<quiche::h3::Connection>,
    inner: ConnectionInner,
    state_notifier: watch::Sender<ConnectionState>,
}

impl QuicheHttp3Connection {
    pub(crate) fn new(
        quiche_conn: quiche::Connection,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        id: ConnectionID,
        h3_event_sender: mpsc::Sender<Http3ConnectionEvent>,
        is_server: bool,
        state_notifier: watch::Sender<ConnectionState>,
    ) -> QuicheHttp3Connection {
        QuicheHttp3Connection {
            quiche_conn,
            h3_conn: None,
            inner: ConnectionInner::new(local_addr, remote_addr, id, h3_event_sender, is_server),
            state_notifier,
        }
    }

    fn create_http3_connection(&mut self) -> Result<(), Http3Error> {
        let config = quiche::h3::Config::new()?;
        let conn = quiche::h3::Connection::with_transport(&mut self.quiche_conn, &config)?;
        self.h3_conn = Some(conn);
        Ok(())
    }

    pub(crate) async fn send_http_request(
        &mut self,
        request: HttpRequest,
    ) -> Result<u64, Http3Error> {
        let h3_conn = match self.h3_conn.as_mut() {
            Some(conn) => conn,
            None => {
                warn!("HTTP/3 connection not created");
                return Err(Http3Error::InvalidConnectionID);
            }
        };

        let mut headers = vec![
            quiche::h3::Header::new(HEADER_METHOD, request.method.to_string().as_bytes()),
            quiche::h3::Header::new(HEADER_PATH, request.uri.as_bytes()),
            quiche::h3::Header::new(HEADER_SCHEME, b"http"),
        ];

        for (name, values) in request.headers.inner {
            for value in values {
                headers.push(quiche::h3::Header::new(name.as_bytes(), value.as_bytes()));
            }
        }

        let has_body = request.body.is_some();
        let fin = !has_body && request.method != Method::Connect;
        let stream_id = h3_conn.send_request(&mut self.quiche_conn, &headers, fin)?;

        if let Some(body) = request.body {
            let bytes = match body {
                Content::Bytes(b) => b,
                Content::File(mut r) => {
                    // TODO: This blocks the event loop by reading the entire stream
                    // into memory. This prevents other events (like ACKs or new requests) from
                    // being processed while reading. Spawn a task to read chunks and feed them to
                    // the connection event loop.
                    let mut b = Vec::new();
                    let _ = r.read_to_end(&mut b).await?;
                    b
                }
            };
            self.send_body_internal(stream_id, bytes, request.method != Method::Connect)?;
        }

        Ok(stream_id)
    }

    fn send_body_internal(
        &mut self,
        stream_id: u64,
        body: Vec<u8>,
        fin: bool,
    ) -> Result<(), Http3Error> {
        let h3_conn = self.h3_conn.as_mut().ok_or(Http3Error::InvalidConnectionID)?;

        if let Some(state) = self.inner.outgoing_streams.get_mut(&stream_id) {
            state.queue.push_back(OutgoingDataChunk { data: body, offset: 0, fin });
            return Ok(());
        }

        // If a partial write happens, quiche will set the fin flag to false internally.
        let sent = match h3_conn.send_body(&mut self.quiche_conn, stream_id, &body, fin) {
            Ok(n) => n,
            Err(quiche::h3::Error::Done) => 0,
            Err(e) => return Err(e.into()),
        };

        // The sending operation is pending until the stream becomes writable.
        if sent < body.len() {
            debug!("[{}] Partial write: {}/{}", stream_id, sent, body.len());
            let mut queue = VecDeque::new();
            queue.push_back(OutgoingDataChunk { data: body, offset: sent, fin });
            let _ = self.inner.outgoing_streams.insert(stream_id, OutgoingStreamState { queue });
        }
        Ok(())
    }

    async fn poll_h3_connection(&mut self) -> Result<(), Http3Error> {
        let h3_conn = match self.h3_conn.as_mut() {
            Some(conn) => conn,
            None => {
                return Err(Http3Error::InternalError(
                    "HTTP/3 connection not established".to_string(),
                ))
            }
        };
        let inner = &mut self.inner;
        let quiche_conn = &mut self.quiche_conn;

        loop {
            match h3_conn.poll(quiche_conn) {
                Ok((stream_id, event)) => match event {
                    quiche::h3::Event::Headers { list, has_body } => {
                        if inner.is_server {
                            inner
                                .handle_request_headers(
                                    stream_id,
                                    list,
                                    has_body,
                                    h3_conn,
                                    quiche_conn,
                                )
                                .await?;
                        } else {
                            inner.handle_response_headers(stream_id, list, has_body).await?;
                        }
                    }
                    quiche::h3::Event::Data => {
                        inner.handle_http_body(stream_id, h3_conn, quiche_conn).await?;
                    }
                    quiche::h3::Event::Finished => {
                        inner.handle_finished(stream_id);
                    }
                    quiche::h3::Event::GoAway => {
                        debug!("[{stream_id}] Received GOAWAY");
                    }
                    _ => {
                        trace!("Got event {event:?}");
                    }
                },
                Err(quiche::h3::Error::Done) => break,
                Err(e) => {
                    debug!("HTTP/3 processing failed: {e:?}");
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn flush_egress(&mut self) -> Result<Vec<u8>, Http3Error> {
        let mut buf = [0; BUFFER_SIZE];

        let (write, _) = match self.quiche_conn.send(&mut buf) {
            Ok(v) => v,
            Err(quiche::Error::Done) => return Err(quiche::Error::Done.into()),
            Err(e) => return Err(e.into()),
        };

        if let Some(data) = buf.get(..write) {
            Ok(data.to_vec())
        } else {
            error!("Buffer size less than data size {write}");
            Err(Http3Error::InternalError(format!("Buffer size less than data size {write}")))
        }
    }

    pub(crate) async fn process_packet(&mut self, data: &mut [u8]) -> Result<(), Http3Error> {
        let recv_info =
            quiche::RecvInfo { from: self.inner.remote_addr, to: self.inner.local_addr };
        let _ = self.quiche_conn.recv(data, recv_info)?;

        if (self.quiche_conn.is_in_early_data() || self.quiche_conn.is_established())
            && self.h3_conn.is_none()
        {
            self.create_http3_connection()?;
            debug!("HTTP/3 connection created");
            self.inner
                .h3_event_sender
                .send(Http3ConnectionEvent::OnConnected {
                    peer_info: crate::PeerInfo {
                        connection_id: self.inner.id.clone(),
                        addr: self.inner.remote_addr,
                    },
                })
                .await?;
            let _ = self.state_notifier.send(ConnectionState::Connected);
        }

        if self.h3_conn.is_some() {
            self.poll_h3_connection().await?;
        }

        Ok(())
    }

    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    pub(crate) fn remote_addr(&self) -> SocketAddr {
        self.inner.remote_addr
    }

    pub(crate) fn connection_id(&self) -> &ConnectionID {
        self.inner.id.as_ref()
    }

    pub(crate) fn timeout(&self) -> Option<Duration> {
        self.quiche_conn.timeout()
    }

    pub(crate) async fn on_timeout(&mut self) {
        self.quiche_conn.on_timeout();
        if self.quiche_conn.is_closed() {
            let is_timed_out = self.quiche_conn.is_timed_out();
            log::info!(
                "[{}] Sending OnDisconnected to {:?}. is_timed_out: {}",
                if self.inner.is_server { "server" } else { "client" },
                self.inner.remote_addr,
                is_timed_out
            );
            let reason = self.quiche_conn.peer_error().map(|e| e.reason.to_vec());
            let _ = self
                .inner
                .h3_event_sender
                .send(Http3ConnectionEvent::OnDisconnected {
                    peer_info: crate::PeerInfo {
                        connection_id: self.inner.id.clone(),
                        addr: self.inner.remote_addr,
                    },
                    reason,
                })
                .await;
        }
    }

    #[allow(dead_code)]
    fn dump_connection_state(&self) {
        trace!(
            "[{}] is_closed {}, is_draining {}, is_established {}",
            if self.inner.is_server { "server" } else { "client" },
            self.quiche_conn.is_closed(),
            self.quiche_conn.is_draining(),
            self.quiche_conn.is_established()
        );
    }

    pub(crate) async fn send_http_response(
        &mut self,
        stream_id: u64,
        response: crate::HttpResponse,
    ) -> Result<(), Http3Error> {
        let h3_conn = match self.h3_conn.as_mut() {
            Some(conn) => conn,
            None => return Err(Http3Error::InvalidConnectionID),
        };

        let mut headers =
            vec![quiche::h3::Header::new(HEADER_STATUS, response.status.to_string().as_bytes())];
        for (name, values) in response.headers.inner {
            for value in values {
                headers.push(quiche::h3::Header::new(name.as_bytes(), value.as_bytes()));
            }
        }

        let has_body = response.body.is_some();
        h3_conn.send_response(&mut self.quiche_conn, stream_id, &headers, !has_body)?;

        if let Some(body) = response.body {
            let bytes = match body {
                Content::Bytes(b) => b,
                Content::File(mut r) => {
                    // TODO: This blocks the event loop by reading the entire stream
                    // into memory. This prevents other events (like ACKs or new requests) from
                    // being processed while reading. Spawn a task to read chunks and feed them to
                    // the connection event loop.
                    let mut b = Vec::new();
                    let _ = r.read_to_end(&mut b).await?;
                    b
                }
            };
            self.send_body_internal(stream_id, bytes, true)?;
        }
        Ok(())
    }

    pub(crate) fn set_control_stream_id(&mut self, stream_id: u64) {
        self.inner.control_stream_id = Some(stream_id);
        self.inner.stream_manager.insert(stream_id, StreamState::ControlStreamConnecting);
    }

    pub(crate) fn send_control_message(&mut self, message: &[u8]) -> Result<(), Http3Error> {
        let stream_id = self.inner.control_stream_id.ok_or_else(|| {
            Http3Error::InternalError("Control stream not established".to_string())
        })?;

        let mut payload = Vec::with_capacity(4 + message.len());
        payload.extend_from_slice(&(message.len() as u32).to_be_bytes());
        payload.extend_from_slice(message);

        self.send_body_internal(stream_id, payload, false)?;
        Ok(())
    }

    pub(crate) fn close(
        &mut self,
        app: bool,
        err: u64,
        reason: &[u8],
    ) -> Result<(), quiche::Error> {
        debug!("[{}] close", if self.inner.is_server { "server" } else { "client" });
        self.quiche_conn.close(app, err, reason)
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.quiche_conn.is_closed()
    }

    pub(crate) fn resume_writes(&mut self) -> Result<(), Http3Error> {
        let writable: Vec<u64> = self.quiche_conn.writable().collect();
        for stream_id in writable {
            if let Some(state) = self.inner.outgoing_streams.get_mut(&stream_id) {
                let h3_conn = self.h3_conn.as_mut().ok_or(Http3Error::InvalidConnectionID)?;
                while let Some(chunk) = state.queue.front_mut() {
                    let body_chunk = chunk.data.get(chunk.offset..).ok_or_else(|| {
                        Http3Error::InternalError("Offset out of bounds".to_string())
                    })?;

                    match h3_conn.send_body(&mut self.quiche_conn, stream_id, body_chunk, chunk.fin)
                    {
                        Ok(sent) => {
                            chunk.offset += sent;
                            debug!("[{}] Resumed write: +{}/{}", stream_id, sent, chunk.data.len());
                            if chunk.offset >= chunk.data.len() {
                                let _ = state.queue.pop_front();
                            } else {
                                // Partial write, still blocked
                                break;
                            }
                        }
                        Err(quiche::h3::Error::Done) => {
                            // Still blocked
                            break;
                        }
                        Err(e) => {
                            error!("Failed to resume write for stream {stream_id}: {e:?}");
                            let _ = self.inner.outgoing_streams.remove(&stream_id);
                            return Err(e.into());
                        }
                    }
                }
                if state.queue.is_empty() {
                    let _ = self.inner.outgoing_streams.remove(&stream_id);
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for QuicheHttp3Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("QuicheHttp3Connection")
            .field("local_addr", &self.local_addr())
            .field("remote_addr", &self.remote_addr())
            .field("conn_id", &self.quiche_conn.trace_id())
            .finish()
    }
}
