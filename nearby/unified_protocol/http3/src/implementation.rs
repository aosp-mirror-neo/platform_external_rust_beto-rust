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

mod client;
mod connection;
mod server;
mod test_certs;

use crate::{Http3Client, Http3Error, Http3Server, PacketReceiver, PacketSender};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

/// Default value for max_idle_timeout transport parameter.
const QUICHE_IDLE_TIMEOUT_MS: u64 = 300_000;

/// Default value for these transport parameters:
/// - initial_max_data
/// - initial_max_stream_data_bidi_local
/// - initial_max_stream_data_bidi_remote
/// - initial_max_stream_data_uni
const QUICHE_MAX_INCOMING_BUFFER_SIZE: u64 = 1000000;

/// Default value for initial_max_streams_bidi transport parameter.
const MAX_CONCURRENT_STREAM_SIZE: u64 = 100;

const QUICHE_UDP_PAYLOAD_SIZE: usize = 1452;

const BUFFER_SIZE: usize = 8192;

const FILE_STREAM_SIZE: usize = 8192;

// TODO: For authentication and dedupping, pass the following as 0-RTT data.
//   1) Session ID (right now it's called dedupe hint)
//   2) identity hint
//   3) Encrypted identity token (A byte array)
//   4) device name + device type
/// Creates a new QuicheHttp3Client to connect to the specified remote peer address.
///
/// # Arguments
///
/// * `socket_addr` - The address of the remote peer to connect to.
pub async fn new_client(
    socket_addr: SocketAddr,
    local_addr: Option<IpAddr>,
) -> Result<Box<dyn Http3Client>, Http3Error> {
    let bind_addr = if let Some(ip) = local_addr {
        SocketAddr::new(ip, 0)
    } else if socket_addr.is_ipv4() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)
    } else {
        SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0)
    };
    let socket = UdpSocket::bind(bind_addr).await?;
    socket.connect(socket_addr).await?;
    new_client_with_socket(Arc::new(socket))
}

/// Creates a new QuicheHttp3Client using an existing UDP socket.
///
/// The socket must be **bound** to a local address and **connected** to a remote peer address.
///
/// # Arguments
///
/// * `socket` - The connected UDP socket.
pub fn new_client_with_socket(socket: Arc<UdpSocket>) -> Result<Box<dyn Http3Client>, Http3Error> {
    let peer_addr = socket.peer_addr()?;
    let local_addr = socket.local_addr()?;
    let sender = Box::new(socket.clone());
    let receiver = Box::new(socket);
    new_client_with_packet_io(sender, receiver, local_addr, peer_addr)
}

/// Creates a new QuicheHttp3Client with custom packet I/O.
///
/// # Arguments
///
/// * `packet_sender` - The interface for sending packets.
/// * `packet_receiver` - The interface for receiving packets.
/// * `local_addr` - The local address of the client.
/// * `peer_addr` - The address of the remote peer.
pub fn new_client_with_packet_io(
    packet_sender: Box<dyn PacketSender>,
    packet_receiver: Box<dyn PacketReceiver>,
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
) -> Result<Box<dyn Http3Client>, Http3Error> {
    let client =
        client::QuicheHttp3Client::new(packet_sender, packet_receiver, local_addr, peer_addr)?;
    Ok(Box::new(client))
}

/// Creates a new QuicheHttp3Server bound to the specified local address.
///
/// # Arguments
///
/// * `socket_addr` - The local address to bind to.
pub async fn new_server(socket_addr: SocketAddr) -> Result<Box<dyn Http3Server>, Http3Error> {
    let socket = Arc::new(UdpSocket::bind(socket_addr).await?);
    new_server_with_socket(socket)
}

/// Creates a new QuicheHttp3Server using an existing UDP socket.
///
/// The socket must be **bound** to a local address.
///
/// # Arguments
///
/// * `socket` - The bound UDP socket.
pub fn new_server_with_socket(socket: Arc<UdpSocket>) -> Result<Box<dyn Http3Server>, Http3Error> {
    // Ensure the socket is bound.
    let local_addr = socket.local_addr()?;
    let sender = Box::new(socket.clone());
    let receiver = Box::new(socket);
    new_server_with_packet_io(sender, receiver, local_addr)
}

/// Creates a new QuicheHttp3Server with custom packet I/O.
///
/// # Arguments
///
/// * `packet_sender` - The interface for sending packets.
/// * `packet_receiver` - The interface for receiving packets.
/// * `local_addr` - The local address the server is bound to.
pub fn new_server_with_packet_io(
    packet_sender: Box<dyn PacketSender>,
    packet_receiver: Box<dyn PacketReceiver>,
    local_addr: SocketAddr,
) -> Result<Box<dyn Http3Server>, Http3Error> {
    let server = server::QuicheHttp3Server::new(packet_sender, packet_receiver, local_addr)?;
    Ok(Box::new(server))
}

#[cfg(test)]
mod tests;
