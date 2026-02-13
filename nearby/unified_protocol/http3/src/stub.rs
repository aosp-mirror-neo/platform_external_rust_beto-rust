// Copyright 2026 Google LLC
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

use crate::{
    Http3Client, Http3ConnectionEvent, Http3Error, Http3Server, HttpRequest, HttpResponse,
    PacketReceiver, PacketSender, PeerInfo,
};
use async_trait::async_trait;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

struct StubHttp3Client;

#[async_trait]
impl Http3Client for StubHttp3Client {
    async fn event(&self) -> Option<Http3ConnectionEvent> {
        None
    }
    async fn request(&self, _request: HttpRequest) -> Result<u64, Http3Error> {
        Err(Http3Error::InternalError("Not implemented".into()))
    }
    async fn send_control_message(&self, _message: &[u8]) -> Result<(), Http3Error> {
        Err(Http3Error::InternalError("Not implemented".into()))
    }
    async fn close(&self, _reason: Option<&[u8]>) -> Result<(), Http3Error> {
        Ok(())
    }
}

struct StubHttp3Server;

#[async_trait]
impl Http3Server for StubHttp3Server {
    async fn event(&self) -> Option<Http3ConnectionEvent> {
        None
    }
    async fn send_response(
        &self,
        _peer_info: PeerInfo,
        _stream_id: u64,
        _response: HttpResponse,
    ) -> Result<(), Http3Error> {
        Err(Http3Error::InternalError("Not implemented".into()))
    }
    async fn send_control_message(
        &self,
        _peer_info: PeerInfo,
        _message: &[u8],
    ) -> Result<(), Http3Error> {
        Err(Http3Error::InternalError("Not implemented".into()))
    }
    async fn close(&self, _reason: Option<&[u8]>) -> Result<(), Http3Error> {
        Ok(())
    }
}

/// Stub implementation.
pub async fn new_client(
    _socket_addr: SocketAddr,
    _local_addr: Option<IpAddr>,
) -> Result<Box<dyn Http3Client>, Http3Error> {
    Ok(Box::new(StubHttp3Client))
}

/// Stub implementation.
pub fn new_client_with_socket(_socket: Arc<UdpSocket>) -> Result<Box<dyn Http3Client>, Http3Error> {
    Ok(Box::new(StubHttp3Client))
}

/// Stub implementation.
pub fn new_client_with_packet_io(
    _packet_sender: Box<dyn PacketSender>,
    _packet_receiver: Box<dyn PacketReceiver>,
    _local_addr: SocketAddr,
    _peer_addr: SocketAddr,
) -> Result<Box<dyn Http3Client>, Http3Error> {
    Ok(Box::new(StubHttp3Client))
}

/// Stub implementation.
pub async fn new_server(_socket_addr: SocketAddr) -> Result<Box<dyn Http3Server>, Http3Error> {
    Ok(Box::new(StubHttp3Server))
}

/// Stub implementation.
pub fn new_server_with_socket(_socket: Arc<UdpSocket>) -> Result<Box<dyn Http3Server>, Http3Error> {
    Ok(Box::new(StubHttp3Server))
}

/// Stub implementation.
pub fn new_server_with_packet_io(
    _packet_sender: Box<dyn PacketSender>,
    _packet_receiver: Box<dyn PacketReceiver>,
    _local_addr: SocketAddr,
) -> Result<Box<dyn Http3Server>, Http3Error> {
    Ok(Box::new(StubHttp3Server))
}
