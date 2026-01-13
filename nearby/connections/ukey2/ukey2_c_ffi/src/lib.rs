// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{HashMap, HashSet};
use std::ptr::null_mut;

use crypto_provider_default::CryptoProviderImpl as CryptoProvider;
use lazy_static::lazy_static;
use lock_adapter::NoPoisonMutex;
use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;

#[cfg(not(feature = "std"))]
use lock_adapter::spin::Mutex;
#[cfg(feature = "std")]
use lock_adapter::stdlib::Mutex;

use ukey2_connections::{
    D2DConnectionContextV1, D2DHandshakeContext, HandleMessageError, HandshakeImplementation,
    InitiatorD2DHandshakeContext, NextProtocol, ServerD2DHandshakeContext,
};

#[repr(C)]
pub struct RustFFIByteArray {
    ptr: *mut u8,
    len: usize,
    cap: usize,
}

impl RustFFIByteArray {
    fn from_vec(vec: Vec<u8>) -> RustFFIByteArray {
        let mut vec = core::mem::ManuallyDrop::new(vec);
        RustFFIByteArray { ptr: vec.as_mut_ptr(), len: vec.len(), cap: vec.capacity() }
    }

    unsafe fn into_vec(self) -> Option<Vec<u8>> {
        if self.ptr.is_null() {
            return None;
        }
        Some(Vec::from_raw_parts(self.ptr, self.len, self.cap))
    }
}

#[repr(C)]
pub struct CFFIByteArray {
    ptr: *mut u8,
    len: usize,
}

#[repr(C)]
pub struct CMessageParseResult {
    success: bool,
    alert_to_send: RustFFIByteArray,
}

type D2DBox = Box<dyn D2DHandshakeContext>;
type ConnectionBox = Box<D2DConnectionContextV1>;

lazy_static! {
    static ref HANDLE_MAPPING: Mutex<HashMap<u64, D2DBox>> = Mutex::new(HashMap::new());
    static ref CONNECTION_HANDLE_MAPPING: Mutex<HashMap<u64, ConnectionBox>> =
        Mutex::new(HashMap::new());
    static ref RNG: Mutex<ChaCha20Rng> = Mutex::new(ChaCha20Rng::from_entropy());
}

fn generate_handle() -> u64 {
    RNG.lock().gen()
}

fn insert_gen_handle(item: D2DBox) -> u64 {
    let handle = generate_handle();
    HANDLE_MAPPING.lock().insert(handle, item);
    handle
}

fn insert_conn_gen_handle(item: ConnectionBox) -> u64 {
    let handle = generate_handle();
    CONNECTION_HANDLE_MAPPING.lock().insert(handle, item);
    handle
}

// Utilities
/// This function deallocates FFIByteArray instances allocated from Rust only.
/// NOTE: Any FFIByteArray instances deallocated by this function will no longer be in a guaranteed
/// usable state.
///
/// # Safety
/// The array must have been allocated by a Rust function with the Rust allocator, e.g.
/// [get_next_handshake_message].
#[no_mangle]
pub unsafe extern "C" fn rust_dealloc_ffi_byte_array(arr: RustFFIByteArray) {
    if let Some(vec) = arr.into_vec() {
        drop(vec);
    }
}

// Common functions
#[no_mangle]
pub extern "C" fn is_handshake_complete(handle: u64) -> bool {
    HANDLE_MAPPING.lock().get(&handle).is_some_and(|ctx| ctx.is_handshake_complete())
}

#[no_mangle]
pub extern "C" fn get_next_handshake_message(handle: u64) -> RustFFIByteArray {
    // TODO: error handling
    let opt_msg = HANDLE_MAPPING.lock().get(&handle).and_then(|c| c.get_next_handshake_message());
    if let Some(msg) = opt_msg {
        RustFFIByteArray::from_vec(msg)
    } else {
        RustFFIByteArray { ptr: null_mut(), len: usize::MAX, cap: usize::MAX }
    }
}

/// # Safety
/// We treat msg as data, so we should never have an issue trying to execute it.
#[no_mangle]
pub unsafe extern "C" fn parse_handshake_message(
    handle: u64,
    arr: CFFIByteArray,
) -> CMessageParseResult {
    let msg = std::slice::from_raw_parts(arr.ptr, arr.len);
    let result = HANDLE_MAPPING.lock().get_mut(&handle).unwrap().handle_handshake_message(msg);
    if let Err(error) = result {
        match error {
            HandleMessageError::InvalidState | HandleMessageError::BadMessage => {
                log::error!("{error:?}");
            }
            HandleMessageError::ErrorMessage(message) => {
                return CMessageParseResult {
                    success: false,
                    alert_to_send: RustFFIByteArray::from_vec(message),
                };
            }
        }
    }
    CMessageParseResult {
        success: true,
        alert_to_send: RustFFIByteArray { ptr: null_mut(), len: usize::MAX, cap: usize::MAX },
    }
}

#[no_mangle]
pub extern "C" fn get_verification_string(handle: u64, length: usize) -> RustFFIByteArray {
    HANDLE_MAPPING
        .lock()
        .get(&handle)
        .map(|h| {
            let auth_vec = h
                .completed_handshake()
                .unwrap()
                .auth_string::<CryptoProvider>()
                .derive_vec(length)
                .unwrap();
            RustFFIByteArray::from_vec(auth_vec)
        })
        .unwrap()
}

#[no_mangle]
pub extern "C" fn to_connection_context(handle: u64) -> u64 {
    // TODO: error handling
    let ctx = HANDLE_MAPPING
        .lock()
        .remove(&handle)
        .map(move |mut ctx| {
            let result = Box::new(ctx.to_connection_context().unwrap());
            drop(ctx);
            result
        })
        .unwrap();
    insert_conn_gen_handle(ctx)
}

#[repr(C)]
pub enum Ukey2HandshakeAlgorithm {
    AesCbcHmacSha256,
    AesGcm256,
}

#[no_mangle]
pub extern "C" fn get_next_protocol(handle: u64) -> Ukey2HandshakeAlgorithm {
    let next_protocol = HANDLE_MAPPING.lock().get(&handle).map(|instance| {
        instance
            .completed_handshake()
            .map_or(NextProtocol::Aes256CbcHmacSha256, |ch| ch.next_protocol)
    });
    match next_protocol {
        Some(protocol) => match protocol {
            NextProtocol::Aes256GcmSiv => Ukey2HandshakeAlgorithm::AesGcm256,
            NextProtocol::Aes256CbcHmacSha256 => Ukey2HandshakeAlgorithm::AesCbcHmacSha256,
        },
        None => Ukey2HandshakeAlgorithm::AesCbcHmacSha256,
    }
}

// Responder-specific functions
/// Creates a new responder-side UKEY2 handshake with the specified cryptographic protocols.
/// # Safety
/// The C++ layer wraps the size and pointer in a vector (so the data is never mismatched),
/// and this pointer and size data does not outlive this function, and is therefore safe to
/// pass on the stack.
#[no_mangle]
pub unsafe extern "C" fn responder_new(
    algorithms: *mut Ukey2HandshakeAlgorithm,
    algorithms_size: u32,
) -> u64 {
    let algo_slice = if algorithms.is_null() || algorithms_size == 0 {
        &[Ukey2HandshakeAlgorithm::AesCbcHmacSha256]
    } else {
        std::slice::from_raw_parts(algorithms, algorithms_size as usize)
    };
    let ctx = Box::new(ServerD2DHandshakeContext::<CryptoProvider>::new(
        HandshakeImplementation::PublicKeyInProtobuf,
        &(Vec::from_iter(
            algo_slice
                .iter()
                .map(|algo| match algo {
                    Ukey2HandshakeAlgorithm::AesCbcHmacSha256 => NextProtocol::Aes256CbcHmacSha256,
                    Ukey2HandshakeAlgorithm::AesGcm256 => NextProtocol::Aes256GcmSiv,
                })
                .collect::<HashSet<_>>()
                .iter()
                .cloned(),
        )),
    ));
    insert_gen_handle(ctx)
}

// Initiator-specific functions

/// Creates a new initiator-side UKEY2 handshake with the specified cryptographic protocols.
/// # Safety
/// The C++ layer wraps the size and pointer in a vector (so the data is never mismatched),
/// and this pointer and size data does not outlive this function, and is therefore safe to
/// pass on the stack.
#[no_mangle]
pub unsafe extern "C" fn initiator_new(
    algorithms: *mut Ukey2HandshakeAlgorithm,
    algorithms_size: u32,
) -> u64 {
    let algo_slice = if algorithms.is_null() || algorithms_size == 0 {
        &[Ukey2HandshakeAlgorithm::AesCbcHmacSha256]
    } else {
        std::slice::from_raw_parts(algorithms, algorithms_size as usize)
    };
    let ctx = Box::new(InitiatorD2DHandshakeContext::<CryptoProvider>::new(
        HandshakeImplementation::PublicKeyInProtobuf,
        Vec::from_iter(
            algo_slice
                .iter()
                .map(|algo| match algo {
                    Ukey2HandshakeAlgorithm::AesCbcHmacSha256 => NextProtocol::Aes256CbcHmacSha256,
                    Ukey2HandshakeAlgorithm::AesGcm256 => NextProtocol::Aes256GcmSiv,
                })
                .collect::<HashSet<_>>()
                .iter()
                .cloned(),
        ),
    ));
    insert_gen_handle(ctx)
}

// Connection Context

/// # Safety
/// We treat msg and associated_data as data, not as executable memory.
/// associated_data and msg are slices so Rust won't try to do anything weird with allocation.
#[no_mangle]
pub unsafe extern "C" fn encode_message_to_peer(
    handle: u64,
    msg: CFFIByteArray,
    associated_data: CFFIByteArray,
) -> RustFFIByteArray {
    if msg.len == 0 {
        return RustFFIByteArray { ptr: null_mut(), len: usize::MAX, cap: usize::MAX };
    }
    let msg = std::slice::from_raw_parts(msg.ptr, msg.len);
    let associated_data = if !associated_data.ptr.is_null() {
        Some(std::slice::from_raw_parts(associated_data.ptr, associated_data.len))
    } else {
        None
    };
    let ret = CONNECTION_HANDLE_MAPPING
        .lock()
        .get_mut(&handle)
        .map(|c| c.encode_message_to_peer::<CryptoProvider>(msg, associated_data));
    if let Some(msg) = ret {
        RustFFIByteArray::from_vec(msg)
    } else {
        log::error!("Was unable to find handle!");
        RustFFIByteArray { ptr: null_mut(), len: usize::MAX, cap: usize::MAX }
    }
}

/// # Safety
/// We treat msg as data, not as executable memory.
#[no_mangle]
pub unsafe extern "C" fn decode_message_from_peer(
    handle: u64,
    msg: CFFIByteArray,
    associated_data: CFFIByteArray,
) -> RustFFIByteArray {
    if msg.len == 0 {
        return RustFFIByteArray { ptr: null_mut(), len: usize::MAX, cap: usize::MAX };
    }
    let msg = std::slice::from_raw_parts(msg.ptr, msg.len);
    let associated_data = if !associated_data.ptr.is_null() {
        Some(std::slice::from_raw_parts(associated_data.ptr, associated_data.len))
    } else {
        None
    };
    let ret: Result<Vec<u8>, ukey2_connections::DecodeError> = CONNECTION_HANDLE_MAPPING
        .lock()
        .get_mut(&handle)
        .unwrap()
        .decode_message_from_peer::<CryptoProvider>(msg, associated_data);
    if let Ok(decoded) = ret {
        RustFFIByteArray::from_vec(decoded)
    } else {
        RustFFIByteArray { ptr: null_mut(), len: usize::MAX, cap: usize::MAX }
    }
}

#[no_mangle]
pub extern "C" fn get_session_unique(handle: u64) -> RustFFIByteArray {
    let session_unique_bytes = CONNECTION_HANDLE_MAPPING
        .lock()
        .get(&handle)
        .unwrap()
        .get_session_unique::<CryptoProvider>();
    RustFFIByteArray::from_vec(session_unique_bytes)
}

#[no_mangle]
pub extern "C" fn get_sequence_number_for_encoding(handle: u64) -> i32 {
    CONNECTION_HANDLE_MAPPING.lock().get(&handle).unwrap().encode_seq_num()
}

#[no_mangle]
pub extern "C" fn get_sequence_number_for_decoding(handle: u64) -> i32 {
    CONNECTION_HANDLE_MAPPING.lock().get(&handle).unwrap().decode_seq_num()
}

#[no_mangle]
pub extern "C" fn save_session(handle: u64) -> RustFFIByteArray {
    let key = CONNECTION_HANDLE_MAPPING.lock().get(&handle).unwrap().save_session();
    RustFFIByteArray::from_vec(key)
}

#[repr(i32)]
#[derive(Debug)]
pub enum Status {
    Good,
    Error,
}

#[repr(C)]
pub struct CD2DRestoreConnectionContextV1Result {
    handle: u64,
    status: Status,
}

/// # Safety
/// We error out if the length is incorrect (too large or too small) for restoring a session.
#[no_mangle]
pub unsafe extern "C" fn from_saved_session(
    arr: CFFIByteArray,
) -> CD2DRestoreConnectionContextV1Result {
    let saved_session = std::slice::from_raw_parts(arr.ptr, arr.len);
    let ctx = D2DConnectionContextV1::from_saved_session::<CryptoProvider>(saved_session);
    if let Ok(conn_ctx) = ctx {
        let final_ctx = Box::new(conn_ctx);
        CD2DRestoreConnectionContextV1Result {
            handle: insert_conn_gen_handle(final_ctx),
            status: Status::Good,
        }
    } else {
        log::error!("failed to restore session with error {:?}", ctx.unwrap_err());
        CD2DRestoreConnectionContextV1Result { handle: u64::MAX, status: Status::Error }
    }
}
