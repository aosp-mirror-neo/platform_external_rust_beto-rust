// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#include <string>

#include "gtest/gtest.h"
#include "ukey2_bindings.h"
#include "ukey2_ffi.h"

namespace rust {
namespace {

// Converts the FFI byte array to a string and frees the underlying memory.
std::string ConvertFFIArrayToString(RustFFIByteArray array) {
    std::string ret = std::string((const char*) array.handle, array.len);
    rust_dealloc_ffi_byte_array(array);
    return ret;
}

CFFIByteArray ConvertStringToByteArray(const std::string& str) {
    return CFFIByteArray {
        .handle = (uint8_t*)str.data(),
        .len = str.length(),
    };
}

void RunHandshake(Ukey2Handshake initiator_handle,
                  Ukey2Handshake responder_handle) {
  ParseResult parse_result = responder_handle.ParseHandshakeMessage(
      initiator_handle.GetNextHandshakeMessage());
  ASSERT_TRUE(parse_result.success);
  parse_result = initiator_handle.ParseHandshakeMessage(
      responder_handle.GetNextHandshakeMessage());
  ASSERT_TRUE(parse_result.success);
  parse_result = responder_handle.ParseHandshakeMessage(
      initiator_handle.GetNextHandshakeMessage());
  ASSERT_TRUE(parse_result.success);
  ASSERT_TRUE(responder_handle.IsHandshakeComplete());
  ASSERT_TRUE(initiator_handle.IsHandshakeComplete());
}

TEST(Ukey2RustTest, HandshakeStartsIncomplete) {
  Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder();
  Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator();
  ASSERT_FALSE(responder_handle.IsHandshakeComplete());
  ASSERT_FALSE(initiator_handle.IsHandshakeComplete());
}

TEST(Ukey2RustTest, HandshakeComplete) {
  Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder();
  Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator();
  RunHandshake(initiator_handle, responder_handle);
}

TEST(Ukey2RustTest, CanSendReceiveMessage) {
  Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder();
  Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator();
  RunHandshake(initiator_handle, responder_handle);
  D2DConnectionContextV1 responder_connection =
      responder_handle.ToConnectionContext();
  D2DConnectionContextV1 initiator_connection =
      initiator_handle.ToConnectionContext();
  std::string message = "hello world";
  auto encoded = responder_connection.EncodeMessageToPeer(message, "assocdata");
  ASSERT_NE(encoded, "");
  auto decoded =
      initiator_connection.DecodeMessageFromPeer(encoded, "assocdata");
  EXPECT_EQ(message, decoded);
}

TEST(Ukey2RustTest, TestSaveRestoreSession) {
  Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder();
  Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator();
  RunHandshake(initiator_handle, responder_handle);
  D2DConnectionContextV1 responder_connection =
      responder_handle.ToConnectionContext();
  D2DConnectionContextV1 initiator_connection =
      initiator_handle.ToConnectionContext();
  auto saved_responder = responder_connection.SaveSession();
  D2DRestoreConnectionContextV1Result restore_result =
      D2DConnectionContextV1::FromSavedSession(saved_responder);
  ASSERT_EQ(restore_result.status,
            CD2DRestoreConnectionContextV1Status::STATUS_GOOD);
  auto new_responder = restore_result.handle;
  std::string encoded = new_responder.EncodeMessageToPeer("hello world", "");
  std::string decoded = initiator_connection.DecodeMessageFromPeer(encoded, "");
  EXPECT_EQ("hello world", decoded);
}

TEST(Ukey2RustTest, CanDoGcmRoundTrip) {
  Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder({Ukey2HandshakeAlgorithm::AES_GCM_256});
  Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator({Ukey2HandshakeAlgorithm::AES_GCM_256});
  RunHandshake(initiator_handle, responder_handle);
  EXPECT_EQ(responder_handle.GetNextProtocol(), Ukey2HandshakeAlgorithm::AES_GCM_256);
  EXPECT_EQ(initiator_handle.GetNextProtocol(), Ukey2HandshakeAlgorithm::AES_GCM_256);
  D2DConnectionContextV1 responder_connection =
      responder_handle.ToConnectionContext();
  D2DConnectionContextV1 initiator_connection =
      initiator_handle.ToConnectionContext();
  std::string message = "hello world";
  auto encoded = responder_connection.EncodeMessageToPeer(message, "assocdata");
  ASSERT_NE(encoded, "");
  auto decoded =
      initiator_connection.DecodeMessageFromPeer(encoded, "assocdata");
  EXPECT_EQ(message, decoded);
}

TEST(Ukey2RustTest, EmptyVectorInitializesCbc) {
    Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder({});
    Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator({});
    RunHandshake(initiator_handle, responder_handle);
    EXPECT_EQ(responder_handle.GetNextProtocol(), Ukey2HandshakeAlgorithm::AES_CBC_HMAC_SHA256);
    EXPECT_EQ(initiator_handle.GetNextProtocol(), Ukey2HandshakeAlgorithm::AES_CBC_HMAC_SHA256);
    D2DConnectionContextV1 responder_connection =
        responder_handle.ToConnectionContext();
    D2DConnectionContextV1 initiator_connection =
        initiator_handle.ToConnectionContext();
    std::string message = "hello world";
    auto encoded = responder_connection.EncodeMessageToPeer(message, "assocdata");
    ASSERT_NE(encoded, "");
    auto decoded =
        initiator_connection.DecodeMessageFromPeer(encoded, "assocdata");
    EXPECT_EQ(message, decoded);
}

TEST(Ukey2RustTest, RepeatedProtocolsInitializesOne) {
    Ukey2Handshake responder_handle = Ukey2Handshake::ForResponder({Ukey2HandshakeAlgorithm::AES_GCM_256, Ukey2HandshakeAlgorithm::AES_GCM_256});
    Ukey2Handshake initiator_handle = Ukey2Handshake::ForInitiator({Ukey2HandshakeAlgorithm::AES_GCM_256, Ukey2HandshakeAlgorithm::AES_GCM_256});
    RunHandshake(initiator_handle, responder_handle);
    EXPECT_EQ(responder_handle.GetNextProtocol(), Ukey2HandshakeAlgorithm::AES_GCM_256);
    EXPECT_EQ(initiator_handle.GetNextProtocol(), Ukey2HandshakeAlgorithm::AES_GCM_256);
    D2DConnectionContextV1 responder_connection =
        responder_handle.ToConnectionContext();
    D2DConnectionContextV1 initiator_connection =
        initiator_handle.ToConnectionContext();
    std::string message = "hello world";
    auto encoded = responder_connection.EncodeMessageToPeer(message, "assocdata");
    ASSERT_NE(encoded, "");
    auto decoded =
        initiator_connection.DecodeMessageFromPeer(encoded, "assocdata");
    EXPECT_EQ(message, decoded);
}

TEST(Ukey2BindingsTest, NullptrInitializesCbc) {
    uint64_t responder_handle = responder_new(/*algorithms= */nullptr, /*size= */0);
    uint64_t initiator_handle = initiator_new(/*algorithms= */nullptr, /*size= */0);
    // Client init generation and parse.
    std::string client_init_msg = ConvertFFIArrayToString(get_next_handshake_message(initiator_handle));
    CMessageParseResult result = parse_handshake_message(responder_handle, ConvertStringToByteArray(client_init_msg));
    ASSERT_TRUE(result.success);
    // Server init generation and parse.
    std::string server_init_msg = ConvertFFIArrayToString(get_next_handshake_message(responder_handle));
    result = parse_handshake_message(initiator_handle, ConvertStringToByteArray(server_init_msg));
    ASSERT_TRUE(result.success);
    // Client finish generation and parse.
    std::string client_finish_msg = ConvertFFIArrayToString(get_next_handshake_message(initiator_handle));
    result = parse_handshake_message(responder_handle, ConvertStringToByteArray(client_finish_msg));
    ASSERT_TRUE(result.success);
    // Confirm handshake complete.
    EXPECT_TRUE(is_handshake_complete(responder_handle));
    EXPECT_TRUE(is_handshake_complete(initiator_handle));
    EXPECT_EQ(get_next_protocol(initiator_handle), Ukey2HandshakeAlgorithm::AES_CBC_HMAC_SHA256);
    EXPECT_EQ(get_next_protocol(initiator_handle), Ukey2HandshakeAlgorithm::AES_CBC_HMAC_SHA256);
}

}  // namespace
}  // namespace rust
