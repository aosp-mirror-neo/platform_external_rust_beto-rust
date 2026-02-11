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

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use crate::{
    new_client, new_client_with_packet_io, new_client_with_socket, new_server,
    new_server_with_packet_io, new_server_with_socket, BodyReader, Content, Headers, Http3Client,
    Http3ConnectionEvent, Http3Error, Http3Server, HttpRequest, HttpResponse, Method,
    PacketReceiver, PacketSender, ProgressReader,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};

struct MockPacketSender {
    /// Map of peer address to the channel sender that transmits packets to that peer.
    peers: HashMap<SocketAddr, mpsc::Sender<(Vec<u8>, SocketAddr)>>,
    /// The address of the sender itself, included in the packet data sent to peers.
    source_addr: SocketAddr,
}

#[async_trait]
impl PacketSender for MockPacketSender {
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> std::io::Result<usize> {
        if let Some(tx) = self.peers.get(&target) {
            tx.send((buf.to_vec(), self.source_addr)).await.unwrap();
            Ok(buf.len())
        } else {
            log::error!("MockPacketSender: No route to {target}");
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "No route"))
        }
    }
}

struct MockPacketReceiver {
    /// Channel receiver for incoming packets.
    rx: Mutex<mpsc::Receiver<(Vec<u8>, SocketAddr)>>,
}

#[async_trait]
impl PacketReceiver for MockPacketReceiver {
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        let mut rx = self.rx.lock().await;
        if let Some((data, addr)) = rx.recv().await {
            let len = std::cmp::min(buf.len(), data.len());
            if let (Some(target), Some(source)) = (buf.get_mut(..len), data.get(..len)) {
                target.copy_from_slice(source);
            }
            Ok((len, addr))
        } else {
            // Emulate pending forever if channel closed,
            // similar to a socket waiting for data.
            std::future::pending().await
        }
    }
}

struct TestClient {
    inner: Box<dyn Http3Client>,
    addr: SocketAddr,
}

/// Allows calling `Http3Client` methods directly on `TestClient`.
impl std::ops::Deref for TestClient {
    type Target = dyn Http3Client;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl std::ops::DerefMut for TestClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.inner
    }
}

struct TestServer {
    inner: Box<dyn Http3Server>,
    addr: SocketAddr,
}

/// Allows calling `Http3Server` methods directly on `TestServer`.
impl std::ops::Deref for TestServer {
    type Target = dyn Http3Server;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl std::ops::DerefMut for TestServer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.inner
    }
}

/// Sets up a mock server and a single client with packet I/O channels.
fn setup_server_and_client() -> (TestServer, TestClient) {
    let client_addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let server_addr: SocketAddr = "127.0.0.1:5678".parse().unwrap();

    // Channels for Client -> Server communication
    let (client_tx, server_rx) = mpsc::channel(100);
    // Channels for Server -> Client communication
    let (server_tx, client_rx) = mpsc::channel(100);

    let client_sender = Box::new(MockPacketSender {
        peers: HashMap::from([(server_addr, client_tx)]),
        source_addr: client_addr,
    });
    let client_receiver = Box::new(MockPacketReceiver { rx: Mutex::new(client_rx) });

    let server_sender = Box::new(MockPacketSender {
        peers: HashMap::from([(client_addr, server_tx)]),
        source_addr: server_addr,
    });
    let server_receiver = Box::new(MockPacketReceiver { rx: Mutex::new(server_rx) });

    let server = new_server_with_packet_io(server_sender, server_receiver, server_addr).unwrap();
    let client =
        new_client_with_packet_io(client_sender, client_receiver, client_addr, server_addr)
            .unwrap();

    (
        TestServer { inner: server, addr: server_addr },
        TestClient { inner: client, addr: client_addr },
    )
}

/// Sets up a mock server and two clients with packet I/O channels.
fn setup_server_and_two_clients() -> (TestServer, TestClient, TestClient) {
    let client1_addr: SocketAddr = "127.0.0.1:10009".parse().unwrap();
    let client2_addr: SocketAddr = "127.0.0.1:10010".parse().unwrap();
    let server_addr: SocketAddr = "127.0.0.1:10011".parse().unwrap();

    // Channels for Clients -> Server communication
    let (clients_to_server_tx, server_rx) = mpsc::channel(100);
    // Channels for Server -> Client 1 communication
    let (server_to_client1_tx, client1_rx) = mpsc::channel(100);
    // Channels for Server -> Client 2 communication
    let (server_to_client2_tx, client2_rx) = mpsc::channel(100);

    let server_sender = Box::new(MockPacketSender {
        peers: HashMap::from([
            (client1_addr, server_to_client1_tx),
            (client2_addr, server_to_client2_tx),
        ]),
        source_addr: server_addr,
    });
    let server_receiver = Box::new(MockPacketReceiver { rx: Mutex::new(server_rx) });

    let client1_sender = Box::new(MockPacketSender {
        peers: HashMap::from([(server_addr, clients_to_server_tx.clone())]),
        source_addr: client1_addr,
    });
    let client1_receiver = Box::new(MockPacketReceiver { rx: Mutex::new(client1_rx) });

    let client2_sender = Box::new(MockPacketSender {
        peers: HashMap::from([(server_addr, clients_to_server_tx.clone())]),
        source_addr: client2_addr,
    });
    let client2_receiver = Box::new(MockPacketReceiver { rx: Mutex::new(client2_rx) });

    let server = new_server_with_packet_io(server_sender, server_receiver, server_addr).unwrap();
    let client1 =
        new_client_with_packet_io(client1_sender, client1_receiver, client1_addr, server_addr)
            .unwrap();
    let client2 =
        new_client_with_packet_io(client2_sender, client2_receiver, client2_addr, server_addr)
            .unwrap();

    (
        TestServer { inner: server, addr: server_addr },
        TestClient { inner: client1, addr: client1_addr },
        TestClient { inner: client2, addr: client2_addr },
    )
}

/// Asserts that an event future resolves to an event that matches the given pattern,
/// and returns the event.
macro_rules! assert_and_get_http3_event {
    ($event_future:expr, $pattern:pat) => {
        async {
            match timeout(Duration::from_secs(1), $event_future).await {
                Ok(Some(actual)) => {
                    if !matches!(actual, $pattern) {
                        panic!("assertion failed: event did not match pattern actual event: `{:?}` expected pattern: `{}`", actual, stringify!($pattern));
                    }
                    Some(actual)
                }
                Ok(None) => None,
                Err(_) => panic!("Timeout waiting for event"),
            }
        }
    };
    ($event_future:expr, $pattern:pat if $guard:expr) => {
        async {
            match timeout(Duration::from_secs(1), $event_future).await {
                Ok(Some(actual)) => {
                    if !matches!(actual, $pattern if $guard) {
                        panic!("assertion failed: event did not match pattern actual event: `{:?}` expected pattern: `{}`", actual, stringify!($pattern if $guard));
                    }
                    Some(actual)
                }
                Ok(None) => None,
                Err(_) => panic!("Timeout waiting for event"),
            }
        }
    };
}

macro_rules! assert_http3_event {
    ($event_future:expr, $pattern:pat) => {
        async {
            let _ = assert_and_get_http3_event!($event_future, $pattern).await;
        }
    };
    ($event_future:expr, $pattern:pat if $guard:expr) => {
        async {
            let _ = assert_and_get_http3_event!($event_future, $pattern if $guard).await;
        }
    };
}

#[tokio::test]
#[ignore]
async fn test_handshake_with_socket_address() {
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9005);
    let server = new_server(server_addr).await.unwrap();
    let client = new_client(server_addr, None).await.unwrap();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(
        client.event(),
        Http3ConnectionEvent::OnConnected {
            peer_info: crate::PeerInfo { addr, .. }
        } if addr == server_addr
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn test_handshake_with_udp_socket() {
    let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_socket.local_addr().unwrap();
    let server = new_server_with_socket(Arc::new(server_socket)).unwrap();

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client_addr = client_socket.local_addr().unwrap();
    client_socket.connect(server_addr).await.unwrap();
    let client = new_client_with_socket(Arc::new(client_socket)).unwrap();

    assert_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnConnected {
            peer_info: crate::PeerInfo { addr, .. }
        } if addr == client_addr
    )
    .await;
    assert_http3_event!(
        client.event(),
        Http3ConnectionEvent::OnConnected {
            peer_info: crate::PeerInfo { addr, .. }
        } if addr == server_addr
    )
    .await;
}

#[tokio::test]
async fn test_handshake_with_mock_packet_io() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnConnected {
            ref peer_info, ..
        } if peer_info.addr == client.addr
    )
    .await;
    assert_http3_event!(
        client.event(),
        Http3ConnectionEvent::OnConnected {
            ref peer_info, ..
        } if peer_info.addr == server.addr
    )
    .await;
}

#[tokio::test]
async fn test_single_get_request() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Client sends a GET request
    let request = HttpRequest {
        method: Method::Get,
        uri: "/test".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let _ = client.request(request).await.unwrap();

    // Server receives the request and sends response.
    if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
        )
        .await
    {
        // Server sends response
        let response = HttpResponse {
            status: 200,
            headers: Headers::new(),
            body: Some(Content::Bytes(b"Hello from server".to_vec())),
        };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    }

    // Client receives the response
    if let Some(Http3ConnectionEvent::OnResponseReceived { mut body_reader, .. }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. })
            .await
    {
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, b"Hello from server");
    }
}

#[tokio::test]
async fn test_single_post_request_with_body() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Client sends POST request
    let post_body = b"Hello from client".to_vec();
    let request = HttpRequest {
        method: Method::Post,
        uri: "/post_test".to_string(),
        headers: Headers::new(),
        body: Some(Content::Bytes(post_body.clone())),
    };
    let client_stream_id = client.request(request).await.unwrap();
    assert_eq!(client_stream_id, 4);

    // Server receives the request and sends response.
    if let Some(Http3ConnectionEvent::OnRequestReceived {
        peer_info,
        stream_id,
        mut body_reader,
        ..
    }) = assert_and_get_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnRequestReceived { method: Method::Post, .. }
    )
    .await
    {
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, post_body);

        // Server sends response
        let response = HttpResponse {
            status: 200,
            headers: Headers::new(),
            body: Some(Content::Bytes(b"POST received".to_vec())),
        };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    }

    // Client receives the response
    if let Some(Http3ConnectionEvent::OnResponseReceived { mut body_reader, .. }) =
        assert_and_get_http3_event!(
            client.event(),
            Http3ConnectionEvent::OnResponseReceived { stream_id: 4, .. }
        )
        .await
    {
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, b"POST received");
    }
}

#[tokio::test]
async fn test_multiple_requests_same_client() {
    let (server, client) = setup_server_and_client();
    let expected_stream_for_req1 = 4;
    let req1 = HttpRequest {
        method: Method::Get,
        uri: "/req1".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let expected_stream_for_req2 = 8;
    let req2_body = b"Hello from client post".to_vec();
    let req2 = HttpRequest {
        method: Method::Post,
        uri: "/req2_post".to_string(),
        headers: Headers::new(),
        body: Some(Content::Bytes(req2_body.clone())),
    };

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Client sends GET request and then POST request.
    let stream_id = client.request(req1).await.unwrap();
    assert_eq!(stream_id, expected_stream_for_req1);
    let stream_id = client.request(req2).await.unwrap();
    assert_eq!(stream_id, expected_stream_for_req2);

    // Server receives the requests and sends responses.
    if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, uri, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnRequestReceived {
                method: Method::Get,
                stream_id,
                ..
            } if stream_id == expected_stream_for_req1
        )
        .await
    {
        assert_eq!(uri, "/req1");
        server
            .send_response(
                peer_info,
                stream_id,
                HttpResponse { status: 200, headers: Headers::new(), body: None },
            )
            .await
            .unwrap();
    }
    if let Some(Http3ConnectionEvent::OnRequestReceived {
        peer_info,
        stream_id,
        uri,
        mut body_reader,
        ..
    }) = assert_and_get_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnRequestReceived {
            method: Method::Post,
            stream_id,
            ..
        } if stream_id == expected_stream_for_req2
    )
    .await
    {
        assert_eq!(uri, "/req2_post");
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, req2_body);

        server
            .send_response(
                peer_info,
                stream_id,
                HttpResponse {
                    status: 201,
                    headers: Headers::new(),
                    body: Some(Content::Bytes(b"POST received".to_vec())),
                },
            )
            .await
            .unwrap();
    }

    // Client receives the responses.
    assert_http3_event!(
        client.event(),
        Http3ConnectionEvent::OnResponseReceived { stream_id, .. } if stream_id == expected_stream_for_req1
    )
    .await;
    assert_http3_event!(
        client.event(),
        Http3ConnectionEvent::OnResponseReceived { stream_id, .. } if stream_id == expected_stream_for_req2
    )
    .await;
}

#[tokio::test]
async fn test_client_close() {
    let (server, client) = setup_server_and_client();
    let close_reason = "client close";

    // Client and server should receive the connect event.
    assert_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnConnected { ref peer_info } if peer_info.addr == client.addr
    )
    .await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { ref peer_info } if peer_info.addr == server.addr ).await;

    // Client closes the connection
    client.close(Some(close_reason.as_bytes())).await.unwrap();

    // Both client and server should see the disconnect event
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnDisconnected { ref peer_info, reason: None } if peer_info.addr == server.addr ).await;
    assert_http3_event!(server.event(), Http3ConnectionEvent::OnDisconnected { ref peer_info, ref reason} if peer_info.addr == client.addr && reason == &Some(close_reason.as_bytes().to_vec())).await;
}

#[tokio::test]
async fn test_server_close() {
    let (server, client1, client2) = setup_server_and_two_clients();

    let close_reason = "client close";

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(
        client1.event(),
        Http3ConnectionEvent::OnConnected { ref peer_info, .. }
            if peer_info.addr == server.addr
    )
    .await;

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client2.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Server closes all connections
    server.close(Some(close_reason.as_bytes())).await.unwrap();

    // Both clients should see the disconnect event
    assert_http3_event!(client1.event(), Http3ConnectionEvent::OnDisconnected { ref peer_info, ref reason} if peer_info.addr == server.addr && reason == &Some(close_reason.as_bytes().to_vec())).await;
    assert_http3_event!(client2.event(), Http3ConnectionEvent::OnDisconnected { ref peer_info, ref reason} if peer_info.addr == server.addr && reason == &Some(close_reason.as_bytes().to_vec())).await;

    // Server should also see the disconnect events
    assert_http3_event!(server.event(), Http3ConnectionEvent::OnDisconnected { reason: None, .. })
        .await;
    assert_http3_event!(server.event(), Http3ConnectionEvent::OnDisconnected { reason: None, .. })
        .await;
}

#[tokio::test]
async fn test_file_transfer_over_http_request() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Create a large-ish file
    let mut file_content = vec![0u8; 1024 * 100]; // 100KB
    for (i, byte) in file_content.iter_mut().enumerate() {
        *byte = (i % 255) as u8;
    }

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), &file_content).unwrap();

    let tokio_file = tokio::fs::File::open(temp_file.path()).await.unwrap();

    let total_progress = Arc::new(AtomicU64::new(0));
    let progress_clone = total_progress.clone();
    let progress_reader = ProgressReader::new(tokio_file, move |n| {
        progress_clone.store(n, Ordering::SeqCst);
    });

    let request = HttpRequest {
        method: Method::Post,
        uri: "/upload_progress".to_string(),
        headers: Headers::new(),
        body: Some(Content::File(BodyReader(Box::pin(progress_reader)))),
    };

    let _ = client.request(request).await.unwrap();

    // Server receives the request
    if let Some(Http3ConnectionEvent::OnRequestReceived {
        peer_info, stream_id, body_reader, ..
    }) = assert_and_get_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnRequestReceived { method: Method::Post, .. }
    )
    .await
    {
        let mut received_body = Vec::new();
        let received_progress = Arc::new(AtomicU64::new(0));
        let progress_clone_rx = received_progress.clone();

        if let Some(reader) = body_reader {
            let mut progress_reader = ProgressReader::new(reader, move |n| {
                progress_clone_rx.store(n, Ordering::SeqCst);
            });
            let _ = progress_reader.read_to_end(&mut received_body).await.unwrap();
        }
        assert_eq!(received_body.len(), file_content.len());
        assert_eq!(received_body, file_content);

        // Check if progress was updated
        assert_eq!(total_progress.load(Ordering::SeqCst), file_content.len() as u64);
        assert_eq!(received_progress.load(Ordering::SeqCst), file_content.len() as u64);

        // Server sends response
        let response = HttpResponse { status: 200, headers: Headers::new(), body: None };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    }

    // Client receives the response
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. }).await;
}

#[tokio::test]
async fn test_file_transfer_over_http_response() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Create a large-ish file
    let mut file_content = vec![0u8; 1024 * 100]; // 100KB
    for (i, byte) in file_content.iter_mut().enumerate() {
        *byte = (i % 255) as u8;
    }

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), &file_content).unwrap();
    let tokio_file = tokio::fs::File::open(temp_file.path()).await.unwrap();

    let server_send_progress = Arc::new(AtomicU64::new(0));
    let server_progress_clone = server_send_progress.clone();
    let progress_reader = ProgressReader::new(tokio_file, move |n| {
        server_progress_clone.store(n, Ordering::SeqCst);
    });

    // Client sends a GET request
    let request = HttpRequest {
        method: Method::Get,
        uri: "/download".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let _ = client.request(request).await.unwrap();

    // Server receives request and sends response with file body
    if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
        )
        .await
    {
        let response = HttpResponse {
            status: 200,
            headers: Headers::new(),
            body: Some(Content::File(BodyReader(Box::pin(progress_reader)))),
        };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    }

    // Client receives response and reads body with progress
    if let Some(Http3ConnectionEvent::OnResponseReceived { body_reader, .. }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. })
            .await
    {
        let mut received_body = Vec::new();
        let client_recv_progress = Arc::new(AtomicU64::new(0));
        let client_progress_clone = client_recv_progress.clone();

        if let Some(reader) = body_reader {
            let mut progress_reader = ProgressReader::new(reader, move |n| {
                client_progress_clone.store(n, Ordering::SeqCst);
            });
            let _ = progress_reader.read_to_end(&mut received_body).await.unwrap();
        }

        assert_eq!(received_body.len(), file_content.len());
        assert_eq!(received_body, file_content);

        // Check if progress was updated
        assert_eq!(server_send_progress.load(Ordering::SeqCst), file_content.len() as u64);
        assert_eq!(client_recv_progress.load(Ordering::SeqCst), file_content.len() as u64);
    }
}

#[tokio::test]
async fn test_long_polling() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // 1. Client sends a long-polling HTTP request
    let request_lp1 = HttpRequest {
        method: Method::Get,
        uri: "/long_poll_1".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let stream_id_lp1 = client.request(request_lp1).await.unwrap();

    // 2. Server holds the request
    let (peer_info_lp1, server_stream_id_lp1) =
        if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, uri, .. }) =
            assert_and_get_http3_event!(
                server.event(),
                Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
            )
            .await
        {
            assert_eq!(uri, "/long_poll_1");
            (peer_info, stream_id)
        } else {
            panic!("Server did not receive LP1");
        };

    // 3. Client sends other non-long-polling HTTP requests
    let normal_request = HttpRequest {
        method: Method::Get,
        uri: "/normal_req".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let normal_stream_id = client.request(normal_request).await.unwrap();

    // 4. Server responds the request (receives normal request first)
    if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, uri, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
        )
        .await
    {
        assert_eq!(uri, "/normal_req");
        let response = HttpResponse {
            status: 200,
            headers: Headers::new(),
            body: Some(Content::Bytes(b"Normal response".to_vec())),
        };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    } else {
        panic!("Server did not receive normal request");
    }

    // Verify client receives normal response
    if let Some(Http3ConnectionEvent::OnResponseReceived { stream_id, mut body_reader, .. }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. })
            .await
    {
        assert_eq!(stream_id, normal_stream_id);
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, b"Normal response");
    }

    // 5. Server sends timeout as the response to the long-polling request
    // Simulate delay
    tokio::time::sleep(Duration::from_millis(100)).await;
    let response_lp1 = HttpResponse {
        status: 408, // Request Timeout (simulated) or just a 200 with empty/specific body
        headers: Headers::new(),
        body: Some(Content::Bytes(b"Timeout/Long poll data".to_vec())),
    };
    server.send_response(peer_info_lp1.clone(), server_stream_id_lp1, response_lp1).await.unwrap();

    // Client receives LP1 response
    if let Some(Http3ConnectionEvent::OnResponseReceived { stream_id, mut body_reader, .. }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. })
            .await
    {
        assert_eq!(stream_id, stream_id_lp1);
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, b"Timeout/Long poll data");
    }

    // 6. Client sends another long-polling HTTP request
    let request_lp3 = HttpRequest {
        method: Method::Get,
        uri: "/long_poll_2".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let _stream_id_lp3 = client.request(request_lp3).await.unwrap();

    // Server receives LP3
    if let Some(Http3ConnectionEvent::OnRequestReceived { uri, .. }) = assert_and_get_http3_event!(
        server.event(),
        Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
    )
    .await
    {
        assert_eq!(uri, "/long_poll_2");
    } else {
        panic!("Server did not receive LP3");
    }
}

#[tokio::test]
async fn test_server_sends_control_message_to_client() {
    let (server, client) = setup_server_and_client();

    let Some(Http3ConnectionEvent::OnConnected { peer_info: client_peer_info }) =
        assert_and_get_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await
    else {
        panic!("Server did not receive OnConnected");
    };

    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Wait for control stream to be established.
    // Client initiates it automatically on connection.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Server sends a control message to client. Verify the client successfully receives the message.
    let message_s2c_1 = b"Control message from server 1".to_vec();
    server.send_control_message(client_peer_info.clone(), &message_s2c_1).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            client.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_s2c_1);
    }

    // 3. Client sends a control message to server. Verify the server successfully receives the message.
    let message_c2s_1 = b"Control message from client 1".to_vec();
    client.send_control_message(&message_c2s_1).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_c2s_1);
    }

    // 4. Client sends a normal HTTP Get request. Verify the client successfully receives the response.
    let request = HttpRequest {
        method: Method::Get,
        uri: "/test".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let _ = client.request(request).await.unwrap();

    // Server receives the request and sends response.
    if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
        )
        .await
    {
        let response = HttpResponse {
            status: 200,
            headers: Headers::new(),
            body: Some(Content::Bytes(b"HTTP Response".to_vec())),
        };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    }

    // Client receives the response
    if let Some(Http3ConnectionEvent::OnResponseReceived { mut body_reader, .. }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. })
            .await
    {
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, b"HTTP Response");
    }

    // 6. Server sends a control message to client again. Verify the client successfully receives the message.
    let message_s2c_2 = b"Control message from server 2".to_vec();
    server.send_control_message(client_peer_info.clone(), &message_s2c_2).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            client.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_s2c_2);
    }

    // 7. Client sends a control message to server again. Verify the server successfully receives the message.
    let message_c2s_2 = b"Control message from client 2".to_vec();
    client.send_control_message(&message_c2s_2).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_c2s_2);
    }
}

#[tokio::test]
async fn test_client_sends_control_message_to_server() {
    let (server, client) = setup_server_and_client();

    let Some(Http3ConnectionEvent::OnConnected { peer_info: client_peer_info }) =
        assert_and_get_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await
    else {
        panic!("Server did not receive OnConnected");
    };

    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Wait for control stream to be established.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Client sends a control message to server. Verify the server successfully receives the message.
    let message_c2s_1 = b"Control message from client 1".to_vec();
    client.send_control_message(&message_c2s_1).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_c2s_1);
    }

    // 3. Server sends a control message to client. Verify the client successfully receives the message.
    let message_s2c_1 = b"Control message from server 1".to_vec();
    server.send_control_message(client_peer_info.clone(), &message_s2c_1).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            client.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_s2c_1);
    }

    // 4. Client sends a normal HTTP Get request. Verify the client successfully receives the response.
    let request = HttpRequest {
        method: Method::Get,
        uri: "/test".to_string(),
        headers: Headers::new(),
        body: None,
    };
    let _ = client.request(request).await.unwrap();

    // Server receives the request and sends response.
    if let Some(Http3ConnectionEvent::OnRequestReceived { peer_info, stream_id, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnRequestReceived { method: Method::Get, .. }
        )
        .await
    {
        let response = HttpResponse {
            status: 200,
            headers: Headers::new(),
            body: Some(Content::Bytes(b"HTTP Response".to_vec())),
        };
        server.send_response(peer_info, stream_id, response).await.unwrap();
    }

    // Client receives the response
    if let Some(Http3ConnectionEvent::OnResponseReceived { mut body_reader, .. }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnResponseReceived { .. })
            .await
    {
        let mut body = Vec::new();
        if let Some(reader) = body_reader.as_mut() {
            let _ = reader.read_to_end(&mut body).await.unwrap();
        }
        assert_eq!(body, b"HTTP Response");
    }

    // 5. Client sends a control message to server again. Verify the server successfully receives the message.
    let message_c2s_2 = b"Control message from client 2".to_vec();
    client.send_control_message(&message_c2s_2).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            server.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_c2s_2);
    }

    // 6. Server sends a control message to client again. Verify the client successfully receives the message.
    let message_s2c_2 = b"Control message from server 2".to_vec();
    server.send_control_message(client_peer_info.clone(), &message_s2c_2).await.unwrap();

    if let Some(Http3ConnectionEvent::OnControlMessageReceived { message: received, .. }) =
        assert_and_get_http3_event!(
            client.event(),
            Http3ConnectionEvent::OnControlMessageReceived { .. }
        )
        .await
    {
        assert_eq!(received, message_s2c_2);
    }
}

#[tokio::test]
async fn test_client_send_connect_request_fails() {
    let (_server, client) = setup_server_and_client();

    let request = HttpRequest {
        method: Method::Connect,
        uri: "/any".to_string(),
        headers: Headers::new(),
        body: None,
    };

    let result = client.request(request).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Http3Error::InvalidRequest(_)));
}

#[tokio::test]
async fn test_control_message_coalescing() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Wait for control stream
    tokio::time::sleep(Duration::from_millis(100)).await;

    let count = 100;
    for i in 0..count {
        let msg = format!("Message {i}").into_bytes();
        client.send_control_message(&msg).await.unwrap();
    }

    for i in 0..count {
        let expected_msg = format!("Message {i}").into_bytes();
        if let Some(Http3ConnectionEvent::OnControlMessageReceived { message, .. }) =
            assert_and_get_http3_event!(
                server.event(),
                Http3ConnectionEvent::OnControlMessageReceived { .. }
            )
            .await
        {
            assert_eq!(message, expected_msg, "Message {i} mismatch");
        }
    }
}

#[tokio::test]
async fn test_control_message_fragmentation() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Wait for control stream
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2000 bytes message (larger than typical MTU ~1452, so it should be fragmented)
    let size = 2000;
    let mut big_msg = Vec::with_capacity(size);
    for i in 0..size {
        big_msg.push((i % 255) as u8);
    }

    // Sending a large message might take some time and multiple packets
    client.send_control_message(&big_msg).await.unwrap();

    // Increase timeout for large message
    let event = timeout(Duration::from_secs(5), server.event()).await;

    match event {
        Ok(Some(Http3ConnectionEvent::OnControlMessageReceived { message, .. })) => {
            assert_eq!(message.len(), size);
            assert_eq!(message, big_msg);
        }
        Ok(other) => panic!("Unexpected event: {other:?}"),
        Err(_) => panic!("Timeout waiting for large control message"),
    }
}

#[tokio::test]
async fn test_control_message_interleaved() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    assert_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await;

    // Wait for control stream
    tokio::time::sleep(Duration::from_millis(100)).await;

    let size = 1500; // 1.5KB
    let mut big_msg = Vec::with_capacity(size);
    for i in 0..size {
        big_msg.push((i % 255) as u8);
    }
    let small_msg = b"End".to_vec();

    client.send_control_message(&big_msg).await.unwrap();
    client.send_control_message(&small_msg).await.unwrap();

    // Expect big message first
    // Use explicit timeout for larger message processing
    let event1 = timeout(Duration::from_secs(5), server.event()).await;
    match event1 {
        Ok(Some(Http3ConnectionEvent::OnControlMessageReceived { message, .. })) => {
            assert_eq!(message.len(), size);
            assert_eq!(message, big_msg);
        }
        Ok(other) => panic!("Unexpected event 1: {other:?}"),
        Err(_) => panic!("Timeout waiting for big message"),
    }

    // Expect small message second
    let event2 = timeout(Duration::from_secs(1), server.event()).await;
    match event2 {
        Ok(Some(Http3ConnectionEvent::OnControlMessageReceived { message, .. })) => {
            assert_eq!(message, small_msg);
        }
        Ok(other) => panic!("Unexpected event 2: {other:?}"),
        Err(_) => panic!("Timeout waiting for small message"),
    }
}

#[tokio::test]
async fn test_client_send_control_message_too_large() {
    let (_server, client) = setup_server_and_client();

    let size = 3000;
    let mut big_msg = Vec::with_capacity(size);
    for i in 0..size {
        big_msg.push((i % 255) as u8);
    }

    let result = client.send_control_message(&big_msg).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Http3Error::InvalidRequest(_)));
}

#[tokio::test]
async fn test_server_send_control_message_too_large() {
    let (server, client) = setup_server_and_client();

    assert_http3_event!(server.event(), Http3ConnectionEvent::OnConnected { .. }).await;
    let Some(Http3ConnectionEvent::OnConnected { peer_info }) =
        assert_and_get_http3_event!(client.event(), Http3ConnectionEvent::OnConnected { .. }).await
    else {
        panic!("Client did not receive OnConnected");
    };

    let size = 3000;
    let mut big_msg = Vec::with_capacity(size);
    for i in 0..size {
        big_msg.push((i % 255) as u8);
    }

    let result = server.send_control_message(peer_info, &big_msg).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Http3Error::InvalidRequest(_)));
}
