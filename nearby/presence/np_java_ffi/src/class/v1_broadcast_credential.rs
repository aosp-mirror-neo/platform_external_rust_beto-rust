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

use jni::{
    objects::{JByteArray, JClass, JObject},
    signature::ReturnType,
    JNIEnv,
};

use np_ffi_core::credentials::V1BroadcastCredential as CoreV1BroadcastCredential;
use pourover::desc::{ClassDesc, FieldDesc};
use pourover::jni_method;

static V1_DISCOVERY_CREDENTIAL_CLS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/credential/V1BroadcastCredential");

/// Rust representation of `class V1BroadcastCredential`.
#[repr(transparent)]
pub struct V1BroadcastCredential<Obj>(pub Obj);

impl<'local, Obj: AsRef<JObject<'local>>> V1BroadcastCredential<Obj> {
    /// Get an array field as a Rust array.
    fn get_array<'env, const N: usize>(
        &self,
        env: &mut JNIEnv<'env>,
        field: &FieldDesc,
    ) -> jni::errors::Result<[u8; N]> {
        let arr: JByteArray<'env> =
            env.get_field_unchecked(self.0.as_ref(), field, ReturnType::Array)?.l()?.into();

        let mut buf = [0; N];
        env.get_byte_array_region(arr, 0, &mut buf[..])?;
        Ok(buf.map(|byte| byte as u8))
    }

    /// Get the key seed.
    pub fn get_key_seed<'env>(&self, env: &mut JNIEnv<'env>) -> jni::errors::Result<[u8; 32]> {
        static KEY_SEED: FieldDesc = V1_DISCOVERY_CREDENTIAL_CLS.field("keySeed", "[B");
        self.get_array(env, &KEY_SEED)
    }

    /// Get the identity token.
    pub fn get_identity_token<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<[u8; 16]> {
        static IDENTITY_TOKEN: FieldDesc = V1_DISCOVERY_CREDENTIAL_CLS.field("identityToken", "[B");
        self.get_array(env, &IDENTITY_TOKEN)
    }

    /// Convert this to the `np_ffi_core` representation.
    pub fn get_as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<CoreV1BroadcastCredential> {
        Ok(CoreV1BroadcastCredential::new(
            self.get_key_seed(env)?,
            self.get_identity_token(env)?.into(),
        ))
    }

    /// Encrypt plaintext metadata with the identity token and key seed of this broadcast credential.
    pub fn encrypt_metadata<'env>(
        &self,
        env: &mut JNIEnv<'env>,
        plaintext_metadata: &[u8],
    ) -> jni::errors::Result<Vec<u8>> {
        let core_credential = self.get_as_core(env)?;
        Ok(core_credential.encrypt_metadata(plaintext_metadata))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust.credential",
    class = "V1BroadcastCredential"
)]
extern "system" fn nativeEncryptMetadata<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    credential: V1BroadcastCredential<JObject<'local>>,
    plaintext_metadata: JByteArray<'local>,
) -> JObject<'local> {
    let Ok(maybe_plaintext_metadata) = optional_byte_array_from_jni(&mut env, plaintext_metadata)
    else {
        return JObject::null();
    };
    let plaintext_metadata = match maybe_plaintext_metadata {
        Some(data) => data,
        None => return JObject::null(),
    };

    let Ok(credential) = credential.get_as_core(&mut env) else {
        return JObject::null();
    };

    let encrypted_metadata_bytes = credential.encrypt_metadata(plaintext_metadata.as_slice());
    env.byte_array_from_slice(&encrypted_metadata_bytes)
        .map(|java_byte_array| java_byte_array.into())
        .unwrap_or_else(|_jni_err| JObject::null())
}

/// Reads a `byte[]` into an `Option<Vec<u8>>`, where `None` represents `null`.
fn optional_byte_array_from_jni<'local>(
    env: &mut JNIEnv<'local>,
    array: impl AsRef<JByteArray<'local>>,
) -> jni::errors::Result<Option<Vec<u8>>> {
    if env.is_same_object(array.as_ref(), JObject::null())? {
        Ok(None)
    } else {
        let bytes = env.convert_byte_array(array)?;
        Ok(Some(bytes))
    }
}
