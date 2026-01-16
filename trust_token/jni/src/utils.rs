// Copyright 2025 Google LLC
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

use core::convert::Into;
use handle_map::{HandleMapDimensions, HandleMapTryAllocateError, HandleNotPresentError};
use jni::{
    objects::{AutoLocal, JByteArray, JList},
    JNIEnv,
};
use pourover::exception::{illegal_state_exception, ThrowableJniError};

pub(crate) fn default_handle_map_dimensions() -> HandleMapDimensions {
    HandleMapDimensions { num_shards: 1, max_active_handles: u32::MAX }
}

pub(crate) fn adapt_handle_map_try_allocate_error<'a>(
    e: HandleMapTryAllocateError<ThrowableJniError<'a>>,
) -> ThrowableJniError<'a> {
    match e {
        HandleMapTryAllocateError::HandleMapFull => {
            illegal_state_exception("too many objects").into()
        }
        HandleMapTryAllocateError::ValueProviderFailed(throwable) => throwable,
    }
}

pub(crate) fn adapt_handle_not_present_error<'a>(
    _: HandleNotPresentError,
) -> ThrowableJniError<'a> {
    illegal_state_exception("invalid handle").into()
}

pub(crate) fn trust_token_invalid_signature_exception(
    msg: impl ToString,
) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/security/trusttoken/TrustTokenInvalidSignatureException".into(),
        msg: msg.to_string(),
    }
}

/// Adds conversions to and from jlong to the Handle type.
macro_rules! jni_handle {
    ( $handle_type_name:ty ) => {
        impl core::convert::From<$handle_type_name> for jni::sys::jlong {
            fn from(handle: $handle_type_name) -> jni::sys::jlong {
                handle
                    .handle_id
                    .try_into()
                    .expect("handle_id is at most max_active_handle(u32::MAX)")
            }
        }

        impl core::convert::From<jni::sys::jlong> for $handle_type_name {
            fn from(id: jni::sys::jlong) -> Self {
                Self { handle_id: id.try_into().expect("handle_id should be positive") }
            }
        }
    };
}
pub(crate) use jni_handle;

/// Maps each element of the ByteArray list with the given function and returns a Vec. It only
/// works for ByteArray list.
pub(crate) fn map_byte_array_list_to_vec<'a, F, E>(
    env: &mut JNIEnv,
    list: &JList,
    action: F,
) -> Result<Vec<E>, ThrowableJniError<'a>>
where
    F: Fn(&[u8]) -> Result<E, ThrowableJniError<'a>>,
{
    let mut ret = Vec::with_capacity(list.size(env)?.try_into().expect("size should be positive"));
    let mut iter = list.iter(env)?;
    while let Some(element) = iter.next(env)? {
        let element: AutoLocal<JByteArray> = env.auto_local(element.into());
        let bytes = env.convert_byte_array(&element)?;
        ret.push(action(bytes.as_slice())?);
    }
    Ok(ret)
}
