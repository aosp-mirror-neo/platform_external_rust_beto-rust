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

//! This crate provides a simple HTTP/3 client and server API.
//!
//! # Reserved URI
//!
//! The URI `/up_http3_ctrl_channel` is reserved for internal control messages.
//! Users should not use this URI for their requests.

#[cfg(feature = "use_quiche_impl")]
mod implementation;
#[cfg(feature = "use_quiche_impl")]
use implementation as backend;

#[cfg(not(feature = "use_quiche_impl"))]
mod stub;
#[cfg(not(feature = "use_quiche_impl"))]
use stub as backend;

pub use backend::{
    new_client, new_client_with_packet_io, new_client_with_socket, new_server,
    new_server_with_packet_io, new_server_with_socket,
};

use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::UdpSocket;

/// A trait for sending UDP packets.
#[async_trait]
pub trait PacketSender: Send + Sync {
    /// Sends data to the given address.
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> std::io::Result<usize>;
}

/// A trait for receiving UDP packets.
#[async_trait]
pub trait PacketReceiver: Send + Sync {
    /// Receives data from the socket.
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)>;
}

#[async_trait]
impl PacketSender for Arc<UdpSocket> {
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> std::io::Result<usize> {
        self.as_ref().send_to(buf, target).await
    }
}

#[async_trait]
impl PacketReceiver for Arc<UdpSocket> {
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        self.as_ref().recv_from(buf).await
    }
}

/// A wrapper around `AsyncRead` that implements `Debug`.
pub struct BodyReader(pub Pin<Box<dyn AsyncRead + Send + Sync>>);

impl fmt::Debug for BodyReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BodyReader")
    }
}

impl AsyncRead for BodyReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.0.as_mut().poll_read(cx, buf)
    }
}

/// A helper struct to track read progress.
pub struct ProgressReader<R, F> {
    inner: R,
    callback: F,
    total_bytes_read: u64,
}

impl<R, F> ProgressReader<R, F> {
    /// Creates a new ProgressReader.
    pub fn new(inner: R, callback: F) -> Self {
        Self { inner, callback, total_bytes_read: 0 }
    }
}

impl<R: AsyncRead + Unpin, F: FnMut(u64) + Unpin> AsyncRead for ProgressReader<R, F> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let me = self.get_mut();
        let before = buf.filled().len();
        let poll = Pin::new(&mut me.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            let after = buf.filled().len();
            let n = after - before;
            if n > 0 {
                me.total_bytes_read += n as u64;
                (me.callback)(me.total_bytes_read);
            }
        }
        poll
    }
}

/// A trait for an HTTP/3 client.
#[async_trait]
pub trait Http3Client: Send + Sync {
    /// Retrieves the next connection event from the stream.
    async fn event(&self) -> Option<Http3ConnectionEvent>;
    /// Sends an HTTP request.
    /// Returns the stream ID used for the request.
    async fn request(&self, request: HttpRequest) -> Result<u64, Http3Error>;
    /// Sends a control message to the peer.
    async fn send_control_message(&self, message: &[u8]) -> Result<(), Http3Error>;
    /// Closes the connection.
    ///
    /// # Arguments
    ///
    /// * `reason` - An optional human-readable reason for the closure.
    async fn close(&self, reason: Option<&[u8]>) -> Result<(), Http3Error>;
}

/// A trait for an HTTP/3 server.
#[async_trait]
pub trait Http3Server: Send + Sync {
    /// Retrieves the next connection event from the stream.
    async fn event(&self) -> Option<Http3ConnectionEvent>;
    /// Sends an HTTP response.
    async fn send_response(
        &self,
        peer_info: PeerInfo,
        stream_id: u64,
        response: HttpResponse,
    ) -> Result<(), Http3Error>;
    /// Sends a control message to the peer.
    async fn send_control_message(
        &self,
        peer_info: PeerInfo,
        message: &[u8],
    ) -> Result<(), Http3Error>;
    /// Closes all active connections.
    ///
    /// # Arguments
    ///
    /// * `reason` - An optional human-readable reason for the closure.
    async fn close(&self, reason: Option<&[u8]>) -> Result<(), Http3Error>;
}

/// Events reported from HTTP/3 server and client.
#[derive(Debug)]
pub enum Http3ConnectionEvent {
    /// The QUIC connection is established.
    OnConnected {
        /// Information about the peer.
        peer_info: PeerInfo,
    },
    /// A HTTP3 request is received.
    ///
    /// This is reported when the HTTP headers are received. If the request contains a body,
    /// the `body_reader` field will be `Some`, and the caller should read from it to receive the data.
    OnRequestReceived {
        /// Information about the peer.
        peer_info: PeerInfo,
        /// The QUIC stream of the HTTP request sent from.
        stream_id: u64,
        /// The HTTP method.
        method: Method,
        /// The request URI.
        uri: String,
        /// The request headers.
        headers: Headers,
        /// The request body reader.
        body_reader: Option<BodyReader>,
    },
    /// A HTTP3 response is received.
    ///
    /// This is reported when the HTTP headers are received. If the response contains a body,
    /// the `body_reader` field will be `Some`, and the caller should read from it to receive the data.
    OnResponseReceived {
        /// Information about the peer.
        peer_info: PeerInfo,
        /// The QUIC stream of the HTTP response sent from.
        stream_id: u64,
        /// The HTTP status code.
        status: u16,
        /// The response headers.
        headers: Headers,
        /// The response body reader.
        body_reader: Option<BodyReader>,
    },
    /// A control message is received.
    OnControlMessageReceived {
        /// Information about the peer.
        peer_info: PeerInfo,
        /// The message received.
        message: Vec<u8>,
    },
    /// The QUIC connection is closed.
    OnDisconnected {
        /// Information about the peer.
        peer_info: PeerInfo,
        /// The reason for the disconnection, if available.
        reason: Option<Vec<u8>>,
    },
}

/// Information about the peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// The connection ID of the peer.
    pub connection_id: ConnectionID,
    /// The address of the peer.
    pub addr: SocketAddr,
}

/// Represents an HTTP Method.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Method {
    /// HTTP GET method.
    Get,
    /// HTTP POST method.
    Post,
    /// For internal use only. Users should not use it.
    Connect,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::Get => write!(f, "GET"),
            Method::Post => write!(f, "POST"),
            Method::Connect => write!(f, "CONNECT"),
        }
    }
}

/// Represents a collection of HTTP Headers.
///
/// Header names are case-insensitive.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Headers {
    /// The header list.
    pub inner: HashMap<String, Vec<String>>,
}

impl Headers {
    /// Creates a new, empty Headers collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a header with the given name and value.
    ///
    /// If a header with the same name already exists, the new value is added to the list of values
    /// for that header.
    pub fn append(&mut self, name: &str, value: &str) {
        self.inner.entry(name.to_lowercase()).or_default().push(value.to_string());
    }

    /// Retrieves the values for a header with the given name.
    ///
    /// Returns `None` if the header is not found.
    pub fn get(&self, name: &str) -> Option<&[String]> {
        self.inner.get(&name.to_lowercase()).map(|v| v.as_slice())
    }
}

/// The content of an HTTP request body or response body.
#[derive(Debug)]
pub enum Content {
    /// The content is a byte array.
    Bytes(Vec<u8>),
    /// The content is a file.
    File(BodyReader),
}

/// Represents an HTTP Request.
#[derive(Debug)]
pub struct HttpRequest {
    /// The HTTP method.
    pub method: Method,
    /// The request URI.
    pub uri: String,
    /// The request headers.
    pub headers: Headers,
    /// The optional request body.
    pub body: Option<Content>,
}

/// Represents an HTTP Response.
#[derive(Debug)]
pub struct HttpResponse {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers.
    pub headers: Headers,
    /// The optional response body.
    pub body: Option<Content>,
}

/// Represents an error that can occur during HTTP/3 operations.
#[derive(Error, Debug)]
pub enum Http3Error {
    /// An error originating from the quiche H3 library.
    #[cfg(feature = "use_quiche_impl")]
    #[error("Quiche H3 Error")]
    QuicheH3Error(#[from] quiche::h3::Error),
    /// An error originating from the quiche library.
    #[cfg(feature = "use_quiche_impl")]
    #[error("Quiche  Error")]
    QuicheError(#[from] quiche::Error),

    /// An I/O error occurred.
    #[error("IO Error")]
    IoError(#[from] std::io::Error),
    /// An error occurred while sending an HTTP/3 event.
    #[error("HTTP/3 event send error")]
    Http3EventSendError(#[source] Box<tokio::sync::mpsc::error::SendError<Http3ConnectionEvent>>),
    /// The destination connection ID is invalid or missing.
    #[error("Invalid or missing Destination Connection ID")]
    InvalidConnectionID,
    /// The request is invalid.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    /// An internal error occurred.
    #[error("Internal error: {0}")]
    InternalError(String),
}

type ConnectionID = Vec<u8>;

/// Maximum size of a control message in bytes.
pub const MAX_CONTROL_MESSAGE_SIZE: usize = 2048;

impl From<tokio::sync::mpsc::error::SendError<Http3ConnectionEvent>> for Http3Error {
    fn from(e: tokio::sync::mpsc::error::SendError<Http3ConnectionEvent>) -> Self {
        Http3Error::Http3EventSendError(Box::new(e))
    }
}
