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

//! JNI bindings for TrustAnchor from the trust_token crate.

use coset::{CborSerializable, CoseKey, CoseKeySet, CoseSign1};
use crypto_provider_default::CryptoProviderImpl;
use handle_map::{declare_handle_map, HandleLike};
use jni::objects::{JClass, JList, JObject};
use jni::sys::jlong;
use jni::JNIEnv;
use pourover::exception::{illegal_argument_exception, try_throwable};
use pourover::jni_method;
use trust_token::TrustAnchor;

use crate::utils::*;

pub(crate) struct TrustAnchorHandle {
    handle_id: u64,
}

jni_handle!(TrustAnchorHandle);
declare_handle_map!(default_handle_map_dimensions(), TrustAnchorHandle, TrustAnchor);

/// Creates a new TrustAnchor struct.
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustAnchor")]
pub extern "system" fn native_init(
    mut env: JNIEnv,
    _class: JClass,
    root_keys: JObject,
    intermediate_certificates: JObject,
) -> jlong {
    try_throwable(&mut env, |env| {
        let root_keys = JList::from_env(env, &root_keys)?;
        let root_key_set = map_byte_array_list_to_vec(env, &root_keys, |encoded| {
            CoseKey::from_slice(encoded).map_err(|err| {
                illegal_argument_exception(format!("failed to parse the root keys: {err}")).into()
            })
        })?;
        let intermediate_certificates = JList::from_env(env, &intermediate_certificates)?;
        let cert_set = map_byte_array_list_to_vec(env, &intermediate_certificates, |encoded| {
            CoseSign1::from_slice(encoded).map_err(|err| {
                illegal_argument_exception(format!(
                    "failed to parse the intermediate certificates: {err}"
                ))
                .into()
            })
        })?;
        TrustAnchorHandle::try_allocate(|| {
            TrustAnchor::new::<CryptoProviderImpl>(&CoseKeySet(root_key_set), cert_set).map_err(
                |e| {
                    illegal_argument_exception(format!("failed to build the trust anchor: {:?}", e))
                        .into()
                },
            )
        })
        .map(|handle| handle.into())
        .map_err(adapt_handle_map_try_allocate_error)
    })
}

/// Returns the function to release a TrustAnchor struct.
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustAnchor")]
pub extern "system" fn native_release(mut env: JNIEnv, _class: JClass, handle_id: jlong) {
    try_throwable(&mut env, |_| {
        let _ = TrustAnchorHandle::deallocate(handle_id.into())
            .map_err(adapt_handle_not_present_error)?;
        Ok(())
    })
}
