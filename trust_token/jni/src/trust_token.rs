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

//! JNI bindings for TrustToken from the trust_token crate.

use std::ffi::c_void;
use std::sync::{Once, OnceLock};

use coset::{CborSerializable, CoseSign1};
use crypto_provider_default::CryptoProviderImpl;
use handle_map::{declare_handle_map, HandleLike};
use jni::objects::{GlobalRef, JByteArray, JClass, JObject};
use jni::sys::{jboolean, jint, jlong, JNI_FALSE, JNI_TRUE, JNI_VERSION_1_6};
use jni::{JNIEnv, JavaVM};
use pourover::exception::{illegal_argument_exception, try_throwable, ThrowableJniError};
use pourover::jni_method;
use trust_token::{ChallengeError, TrustTokenPolicy};
use trust_token::{TrustToken, TrustTokenError};

use crate::trust_anchor::TrustAnchorHandle;
use crate::utils::*;

pub(crate) struct TrustTokenHandle {
    handle_id: u64,
}

jni_handle!(TrustTokenHandle);
declare_handle_map!(default_handle_map_dimensions(), TrustTokenHandle, TrustToken);

static INIT: Once = Once::new();

static VERIFIED_IDENTITY: OnceLock<GlobalRef> = OnceLock::new();
static VERIFIED_IDENTITY_STRONG: OnceLock<GlobalRef> = OnceLock::new();
static VERIFIED_DEVICE: OnceLock<GlobalRef> = OnceLock::new();
static VERIFIED_DEVICE_STRONG: OnceLock<GlobalRef> = OnceLock::new();

#[allow(non_snake_case)]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut c_void) -> jint {
    INIT.call_once(|| {
        (|| -> jni::errors::Result<()> {
            let mut env = vm.get_env().expect("Cannot get reference to the JNIEnv");
            let policy =
                env.find_class("com/google/android/security/trusttoken/TrustTokenPolicy")?;
            let mut get_policy_enum = |enum_name| {
                let field: JObject = env
                    .get_static_field(
                        &policy,
                        enum_name,
                        "Lcom/google/android/security/trusttoken/TrustTokenPolicy;",
                    )?
                    .try_into()?;
                env.new_global_ref(field)
            };
            // JNI_OnLoad should be called only once, but there can be bugs like b/220523932. We
            // ignore the error if the values are already initialized to avoid crashing the system.
            let _ = VERIFIED_IDENTITY.set(get_policy_enum("VERIFIED_IDENTITY")?);
            let _ = VERIFIED_IDENTITY_STRONG.set(get_policy_enum("VERIFIED_IDENTITY_STRONG")?);
            let _ = VERIFIED_DEVICE.set(get_policy_enum("VERIFIED_DEVICE")?);
            let _ = VERIFIED_DEVICE_STRONG.set(get_policy_enum("VERIFIED_DEVICE_STRONG")?);
            Ok(())
        })()
        .expect("caching JNI objects should succeed")
    });
    // This is the highest version supported by Android.
    JNI_VERSION_1_6
}

/// Creates a new TrustToken struct.
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_init(
    mut env: JNIEnv,
    _class: JClass,
    trust_anchor_handle: jlong,
    encoded: JByteArray,
) -> jlong {
    try_throwable(&mut env, |env| {
        let trust_anchor_handle = trust_anchor_handle.into();
        let trust_anchor =
            TrustAnchorHandle::get(&trust_anchor_handle).map_err(adapt_handle_not_present_error)?;
        env.convert_byte_array(&encoded)
            .map_err(ThrowableJniError::from)
            .and_then(|encoded| {
                CoseSign1::from_slice(encoded.as_slice()).map_err(|err| {
                    illegal_argument_exception(format!(
                        "failed to parse the encoded CoseSign1 when parsing the token: {err:?}"
                    ))
                    .into()
                })
            })
            .and_then(|cose_encoded| {
                TrustTokenHandle::try_allocate(|| {
                    TrustToken::from_encoded::<CryptoProviderImpl>(cose_encoded, &trust_anchor)
                        .map_err(|e| {
                            match e {
                                TrustTokenError::SignatureError(inner_err) => {
                                    trust_token_invalid_signature_exception(format!(
                                        "the trust token is not signed correctly: {:?}",
                                        inner_err
                                    ))
                                }
                                _ => illegal_argument_exception(format!(
                                    "invalid trust token: {:?}",
                                    e
                                )),
                            }
                            .into()
                        })
                })
                .map_err(adapt_handle_map_try_allocate_error)
            })
            .map(|handle| handle.into())
    })
}

/// Returns the function to release a TrustToken struct.
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_release(mut env: JNIEnv, _class: JClass, handle_id: jlong) {
    try_throwable(&mut env, |_| {
        let _ = TrustTokenHandle::deallocate(handle_id.into())
            .map_err(adapt_handle_not_present_error)?;
        Ok(())
    })
}

/// The binding for TrustToken::get_expiration_time_millis.
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_get_expiration_time_millis(
    mut env: JNIEnv,
    _class: JClass,
    handle_id: jlong,
) -> jlong {
    try_throwable(&mut env, |_env| {
        let handle_id = handle_id.into();
        let token = TrustTokenHandle::get(&handle_id).map_err(adapt_handle_not_present_error)?;
        Ok((token.expiration_time().0 as jlong) * 1000)
    })
}

/// The bindings for TrustToken::get_issued_at_millis
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_get_issued_at_millis(
    mut env: JNIEnv,
    _class: JClass,
    handle_id: jlong,
) -> jlong {
    try_throwable(&mut env, |_env| {
        let handle_id = handle_id.into();
        let token = TrustTokenHandle::get(&handle_id).map_err(adapt_handle_not_present_error)?;
        Ok((token.issued_at().0 as jlong) * 1000)
    })
}

/// The bindings for TrustToken::get_public_key
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_get_public_key<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    handle_id: jlong,
) -> JByteArray<'local> {
    try_throwable(&mut env, |env| {
        let handle_id = handle_id.into();
        let token = TrustTokenHandle::get(&handle_id).map_err(adapt_handle_not_present_error)?;
        env.byte_array_from_slice(token.public_key_bytes()).map_err(|err| err.into())
    })
}

/// The bindings for TrustToken::get_policy
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_get_policy<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    handle_id: jlong,
) -> JObject<'local> {
    try_throwable(&mut env, |env| {
        let handle_id = handle_id.into();
        let token = TrustTokenHandle::get(&handle_id).map_err(adapt_handle_not_present_error)?;
        let policy = match token.policy() {
            TrustTokenPolicy::VerifiedIdentity => VERIFIED_IDENTITY.get(),
            TrustTokenPolicy::VerifiedIdentityStrong => VERIFIED_IDENTITY_STRONG.get(),
            TrustTokenPolicy::VerifiedDevice => VERIFIED_DEVICE.get(),
            TrustTokenPolicy::VerifiedDeviceStrong => VERIFIED_DEVICE_STRONG.get(),
        }
        .expect("Java enum objects should be cached");
        // Make a local reference as try_throwable doesn't support returning a GlobalRef.
        env.new_local_ref(policy).map_err(Into::into)
    })
}

/// The bindings for TrustToken::verify_challenge
#[jni_method(package = "com.google.android.security.trusttoken", class = "TrustToken")]
pub extern "system" fn native_verify_challenge<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    handle_id: jlong,
    challenge: JByteArray,
    remote_response: JByteArray,
) -> jboolean {
    try_throwable(&mut env, |env| {
        let handle_id = handle_id.into();
        let token = TrustTokenHandle::get(&handle_id).map_err(adapt_handle_not_present_error)?;
        let challenge = env.convert_byte_array(&challenge).map_err(ThrowableJniError::from)?;
        let remote_response =
            env.convert_byte_array(&remote_response).map_err(ThrowableJniError::from)?;
        match token.verify_challenge::<CryptoProviderImpl>(&challenge, &remote_response) {
            Ok(_) => Ok(JNI_TRUE),
            Err(err) => match err {
                ChallengeError::InvalidKey(e) => {
                    Err(illegal_argument_exception(e.to_string()).into())
                }
                ChallengeError::WrongResponse => Ok(JNI_FALSE),
            },
        }
    })
}
