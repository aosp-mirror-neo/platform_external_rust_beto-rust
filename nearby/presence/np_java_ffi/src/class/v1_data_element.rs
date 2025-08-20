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

//! Data Elements for v1 advertisements. See `class V1DataElement`.

use crate::exception_wrapper;
use array_view::ArrayView;
use jni::{
    objects::{JByteArray, JClass, JObject, JThrowable},
    sys::{jbyte, jint, jlong},
    JNIEnv,
};
use np_ffi_core::{common::ByteBuffer, v1::GenericV1DataElement};
use pourover::desc::ClassDesc;
use pourover::jni_method;

exception_wrapper!(
    /// Rust representation of `class V1DataElement.TooMuchDataException`
    TooMuchDataException, "com/google/android/nearby/presence/rust/V1DataElement$TooMuchDataException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.TxPowerOutOfBoundsException`
    TxPowerOutOfBoundsException, "com/google/android/nearby/presence/rust/V1DataElement$TxPowerOutOfBoundsException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.TxPowerWrongLengthException`
    TxPowerWrongLengthException, "com/google/android/nearby/presence/rust/V1DataElement$TxPowerWrongLengthException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.CastIdWrongLengthException`
    CastIdWrongLengthException, "com/google/android/nearby/presence/rust/V1DataElement$CastIdWrongLengthException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.ActionsMalformedException`
    ActionsMalformedException, "com/google/android/nearby/presence/rust/V1DataElement$ActionsMalformedException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.ContextSyncSeqNumWrongLengthException`
    ContextSyncSeqNumWrongLengthException, "com/google/android/nearby/presence/rust/V1DataElement$ContextSyncSeqNumWrongLengthException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.ContextSyncSeqNumOutOfBoundsException`
    ContextSyncSeqNumOutOfBoundsException, "com/google/android/nearby/presence/rust/V1DataElement$ContextSyncSeqNumOutOfBoundsException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.DedupHintWrongLengthException`
    DedupHintWrongLengthException, "com/google/android/nearby/presence/rust/V1DataElement$DedupHintWrongLengthException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.DeviceTypeWrongLengthException`
    DeviceTypeWrongLengthException, "com/google/android/nearby/presence/rust/V1DataElement$DeviceTypeWrongLengthException");

exception_wrapper!(
    /// Rust representation of `class V1DataElement.ConnectivityInfoMalformedException`
    ConnectivityInfoMalformedException, "com/google/android/nearby/presence/rust/V1DataElement$ConnectivityInfoMalformedException");
exception_wrapper!(
    /// Rust representation of `class V1DataElement.MediaDeduplicationIdWrongLengthException`
    MediaDeduplicationIdWrongLengthException, "com/google/android/nearby/presence/rust/V1DataElement$MediaDeduplicationIdWrongLengthException");
exception_wrapper!(
    /// Rust representation of `class V1DataElement.CapabilitiesMalformedException`
    CapabilitiesMalformedException, "com/google/android/nearby/presence/rust/V1DataElement$CapabilitiesMalformedException");
exception_wrapper!(
    /// Rust representation of `class V1DataElement.RequirementsMalformedException`
    RequirementsMalformedException, "com/google/android/nearby/presence/rust/V1DataElement$RequirementsMalformedException");

static CONNECTIVITY_INFO_MALFORMED_COMPONENT_EXCEPTION_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$ConnectivityInfoComponentMalformedException");

static GENERIC_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$Generic");

static TX_POWER_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$TxPower");

static CAST_ID_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$CastId");

static MEDIA_DEDUPLICATION_ID_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$MediaDeduplicationId");

static ACTIONS_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$Actions");

static CONTEXT_SYNC_SEQ_NUM_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$ContextSyncSeqNum");

static DEDUP_HINT_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$DedupHint");

static DEVICE_TYPE_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$DeviceType");

static CONNECTIVITY_INFO_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$ConnectivityInfo");

static BLE_CONNECTIVITY_INFO_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$BleConnectivityInfo");

static WIFI_LAN_CONNECTIVITY_INFO_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$WifiLanConnectivityInfo");

static CAPABILITIES_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$Capabilities");

static REQUIREMENTS_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/nearby/presence/rust/V1DataElement$Requirements");

/// Rust representation of `class V1DataElement.Generic`.
#[repr(transparent)]
pub struct Generic<'local>(pub JObject<'local>);

impl<'local> Generic<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::GenericV1DataElement`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        generic_de: np_ffi_core::v1::GenericV1DataElement,
    ) -> jni::errors::Result<Option<Self>> {
        let de_type = jlong::from(generic_de.de_type().to_u32());
        let Ok(slice) = generic_de.payload.as_slice() else {
            // No JNI errors, but the stack data structure was invalid.
            return Ok(None);
        };

        let payload = env.auto_local(env.byte_array_from_slice(slice)?);
        let result =
            pourover::call_constructor!(env, &GENERIC_CLASS, "(J[B)V", de_type, payload.as_ref())?;
        Ok(Some(Self(result)))
    }
}

impl<'local> Generic<'local> {
    /// Get a reference to the inner `jni` crate [`JObject`].
    pub fn as_obj(&self) -> &JObject<'local> {
        &self.0
    }

    /// Get the data element's type
    pub fn get_type<'env>(&self, env: &mut JNIEnv<'env>) -> jni::errors::Result<jlong> {
        pourover::call_method!(env, &GENERIC_CLASS, "getType", "()J", self.as_obj())
    }

    /// Get the data element's data. Returns `None` if the data is too long.
    pub fn get_data<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<ByteBuffer<127>>> {
        let data = pourover::call_method!(env, &GENERIC_CLASS, "getData", "()[B", self.as_obj())?;
        let len = env.get_array_length(&data)? as usize;

        if len > 127 {
            return Ok(None);
        }

        // Length is validated above
        #[allow(clippy::unwrap_used)]
        {
            let mut buffer = [0; 127];
            env.get_byte_array_region(&data, 0, buffer.get_mut(..len).unwrap())?;
            let buffer = buffer.map(|b| b as u8);
            Ok(Some(ByteBuffer::from_array_view(ArrayView::try_from_array(buffer, len).unwrap())))
        }
    }

    /// Get the data element as a `np_ffi_core` generic DE. Returns `None` if the data element is
    /// not valid.
    pub fn as_core_generic_de<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<GenericV1DataElement>> {
        let Some(payload) = self.get_data(env)? else {
            return Ok(None);
        };
        let de_type = self.get_type(env)? as u32;
        let Ok(de_type): Result<np_adv::extended::de_type::DeType, _> = de_type.try_into() else {
            return Ok(None);
        };
        let de_type = de_type.into();
        Ok(Some(GenericV1DataElement { de_type, payload }))
    }
}

fn translate_deserialized_de<'local>(
    env: &mut JNIEnv<'local>,
    de: np_ffi_core::v1::V1DataElement,
) -> JObject<'local> {
    use np_ffi_core::v1::V1DataElement;
    let result: jni::errors::Result<Option<JObject<'local>>> = match de {
        V1DataElement::Generic(generic) => Generic::construct(env, generic).map(|x| x.map(|y| y.0)),
        V1DataElement::CastId(cast_id) => CastId::construct(env, cast_id).map(|x| Some(x.0)),
        V1DataElement::MediaDeduplicationId(media_deduplication_id) => {
            MediaDeduplicationId::construct(env, media_deduplication_id).map(|x| Some(x.0))
        }
        V1DataElement::Actions(actions) => Actions::construct(env, actions).map(|x| Some(x.0)),
        V1DataElement::TxPower(tx_power) => TxPower::construct(env, tx_power).map(|x| Some(x.0)),
        V1DataElement::ContextSyncSeqNum(seq_num) => {
            ContextSyncSeqNum::construct(env, seq_num).map(|x| Some(x.0))
        }
        V1DataElement::DedupHint(dedup_hint) => {
            DedupHint::construct(env, dedup_hint).map(|x| Some(x.0))
        }
        V1DataElement::DeviceType(device_type) => {
            DeviceType::construct(env, device_type).map(|x| Some(x.0))
        }
        V1DataElement::ConnectivityInfo(connectivity_info) => {
            ConnectivityInfo::construct(env, connectivity_info).map(|x| Some(x.0))
        }
        V1DataElement::Capabilities(capabilities) => {
            Capabilities::construct(env, capabilities).map(|x| Some(x.0))
        }
        V1DataElement::Requirements(requirements) => {
            Requirements::construct(env, requirements).map(|x| Some(x.0))
        }
    };
    match result {
        Ok(Some(result)) => result,
        _ => {
            // Either the deser'ed DE was a malformed stack data structure
            // (shouldn't happen), or there was an unrecoverable JNI error.
            JObject::null()
        }
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.Generic",
    method_name = "nativeTryDeserialize"
)]
extern "system" fn try_deserialize_generic_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: Generic<'local>,
) -> JObject<'local> {
    let generic_de = match this.as_core_generic_de(&mut env) {
        Ok(Some(generic_de)) => generic_de,
        Err(_) => {
            // Internal JNI error
            return JObject::null();
        }
        Ok(None) => {
            // Invalid Generic DE Java structure
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Invalid Generic DE structure, payload likely too long.",
            );
            return JObject::null();
        }
    };
    let Ok(result) = generic_de.try_deserialize() else {
        // Invalid stack data structure, but this
        // shouldn't happen if the Java structure was ok.
        return JObject::null();
    };
    use np_ffi_core::deserialize::v1::V1DataElementDeserializationResult;
    match result {
        V1DataElementDeserializationResult::Success(de) => {
            return translate_deserialized_de(&mut env, de);
        }
        V1DataElementDeserializationResult::TxPowerOutOfBounds => {
            let _ = TxPowerOutOfBoundsException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::TxPowerWrongLength => {
            let _ = TxPowerWrongLengthException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::CastIdWrongLength => {
            let _ = CastIdWrongLengthException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::ActionsMalformed => {
            let _ = ActionsMalformedException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::ContextSyncSeqNumWrongLength => {
            let _ = ContextSyncSeqNumWrongLengthException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::ContextSyncSeqNumOutOfBounds => {
            let _ = ContextSyncSeqNumOutOfBoundsException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::DedupHintWrongLength => {
            let _ = DedupHintWrongLengthException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::DeviceTypeWrongLength => {
            let _ = DeviceTypeWrongLengthException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::ConnectivityInfoMalformed => {
            let _ = ConnectivityInfoMalformedException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::ConnectivityInfoMalformedComponent(index) => {
            let Ok(ex) = pourover::call_constructor!(
                &mut env,
                &CONNECTIVITY_INFO_MALFORMED_COMPONENT_EXCEPTION_CLASS,
                "(I)V",
                index as jni::sys::jint
            ) else {
                // JNI be buggin'
                return JObject::null();
            };
            let _ = env.throw(<&JThrowable>::from(ex.as_ref()));
        }
        V1DataElementDeserializationResult::MediaDeduplicationIdWrongLength => {
            let _ = MediaDeduplicationIdWrongLengthException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::CapabilitiesMalformed => {
            let _ = CapabilitiesMalformedException::throw_new(&mut env);
        }
        V1DataElementDeserializationResult::RequirementsMalformed => {
            let _ = RequirementsMalformedException::throw_new(&mut env);
        }
    }
    // Catch-all return value for thrown errors.
    JObject::null()
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.Generic",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_generic_de<'local>(
    _env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: Generic<'local>,
) -> Generic<'local> {
    this
}

/// Rust representation of `class V1DataElement.TxPower`.
#[repr(transparent)]
pub struct TxPower<'local>(pub JObject<'local>);

impl<'local> TxPower<'local> {
    /// Create a new Java instance from the given `np_ffi_core::common::TxPower`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        tx_power: np_ffi_core::common::TxPower,
    ) -> jni::errors::Result<Self> {
        let tx_power = tx_power.as_i8() as jint;
        pourover::call_constructor!(env, &TX_POWER_CLASS, "(I)V", tx_power).map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::common::TxPower`.
    /// Since the Java representation is not range-restricted, this method returns
    /// a wrapped `None` in the case where the value was out of the accepted range.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::common::TxPower>> {
        let tx_power = pourover::call_method!(env, &TX_POWER_CLASS, "getTxPower", "()I", &self.0)?;
        let Ok(tx_power) = i8::try_from(tx_power) else {
            return Ok(None);
        };
        let np_ffi_core::common::BuildTxPowerResult::Success(tx_power) =
            np_ffi_core::common::TxPower::build_from_signed_byte(tx_power)
        else {
            return Ok(None);
        };
        Ok(Some(tx_power))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.TxPower",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_tx_power_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: TxPower<'local>,
) -> JObject<'local> {
    let Ok(maybe_tx_power) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    let maybe_generic_de = maybe_tx_power.and_then(|tx_power| tx_power.try_serialize_v1().ok());
    try_construct_generic_de_from_serialized(&mut env, maybe_generic_de, "Invalid Tx Power DE")
}

/// Given a possibly-successfully-serialized `GenericV1DataElement`
/// stemming from some other DE, return it as a `class DataElement.Generic`,
/// or throw an `IllegalStateException` if serialization failed due
/// to the original structure being ill-formed.
fn try_construct_generic_de_from_serialized<'local>(
    env: &mut JNIEnv<'local>,
    maybe_serialized_de: Option<GenericV1DataElement>,
    malformed_data_structure_message: &str,
) -> JObject<'local> {
    match maybe_serialized_de {
        None => {
            let _ =
                env.throw_new("java/lang/IllegalStateException", malformed_data_structure_message);
            JObject::null()
        }
        Some(generic_de) => {
            let Ok(Some(generic_de)) = Generic::construct(env, generic_de) else {
                // Either a serialization routine yields borked stack data structures,
                // or the JNI has hit an internal error.
                return JObject::null();
            };
            generic_de.0
        }
    }
}

/// Rust representation of `class V1DataElement.CastId`.
#[repr(transparent)]
pub struct CastId<'local>(pub JObject<'local>);

impl<'local> CastId<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::CastId`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        cast_id: np_ffi_core::v1::CastId,
    ) -> jni::errors::Result<Self> {
        let cast_id = env.auto_local(env.byte_array_from_slice(&cast_id.bytes)?);
        pourover::call_constructor!(env, &CAST_ID_CLASS, "([B)V", cast_id.as_ref()).map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::CastId`.
    /// Since the Java representation is not length-restricted, this method
    /// returns a wrapped `None` in the case where the value did not have
    /// an acceptable length.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::CastId>> {
        let bytes = pourover::call_method!(env, &CAST_ID_CLASS, "getCastId", "()[B", &self.0)?;
        let bytes = env.convert_byte_array(bytes)?;
        let Ok(bytes) = <[u8; 32]>::try_from(bytes) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::CastId { bytes }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.CastId",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_cast_id_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: CastId<'local>,
) -> JObject<'local> {
    let Ok(maybe_cast_id) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    try_construct_generic_de_from_serialized(
        &mut env,
        maybe_cast_id.map(|cast_id| cast_id.serialize()),
        "Invalid Cast Id DE",
    )
}

/// Rust representation of `class V1DataElement.MediaDeduplicationId`.
#[repr(transparent)]
pub struct MediaDeduplicationId<'local>(pub JObject<'local>);

impl<'local> MediaDeduplicationId<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::MediaDeduplicationId`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        media_deduplication_id: np_ffi_core::v1::MediaDeduplicationId,
    ) -> jni::errors::Result<Self> {
        let media_deduplication_id =
            env.auto_local(env.byte_array_from_slice(&media_deduplication_id.bytes)?);
        pourover::call_constructor!(
            env,
            &MEDIA_DEDUPLICATION_ID_CLASS,
            "([B)V",
            media_deduplication_id.as_ref()
        )
        .map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::MediaDeduplicationId`.
    /// Since the Java representation is not length-restricted, this method
    /// returns a wrapped `None` in the case where the value did not have
    /// an acceptable length.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::MediaDeduplicationId>> {
        let bytes = pourover::call_method!(
            env,
            &MEDIA_DEDUPLICATION_ID_CLASS,
            "getMediaDeduplicationId",
            "()[B",
            &self.0
        )?;
        let bytes = env.convert_byte_array(bytes)?;
        let Ok(bytes) = <[u8; 20]>::try_from(bytes) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::MediaDeduplicationId { bytes }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.MediaDeduplicationId",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_media_deduplication_id_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: MediaDeduplicationId<'local>,
) -> JObject<'local> {
    let Ok(maybe_media_deduplication_id) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    try_construct_generic_de_from_serialized(
        &mut env,
        maybe_media_deduplication_id
            .map(|media_deduplication_id| media_deduplication_id.serialize()),
        "Invalid Media Deduplication Id DE",
    )
}

/// Rust representation of `class V1DataElement.Actions`.
#[repr(transparent)]
pub struct Actions<'local>(pub JObject<'local>);

impl<'local> Actions<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::V1ActionsBitmap`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        bitmap: np_ffi_core::v1::V1ActionsBitmap,
    ) -> jni::errors::Result<Self> {
        let bitmap = env.auto_local(env.byte_array_from_slice(&bitmap.bitmap)?);
        pourover::call_constructor!(env, &ACTIONS_CLASS, "([B)V", bitmap.as_ref()).map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::Actions`.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::V1ActionsBitmap>> {
        let bitmap = pourover::call_method!(env, &ACTIONS_CLASS, "getRawBitSet", "()[B", &self.0)?;
        let bitmap = env.convert_byte_array(bitmap)?;
        let Ok(bitmap) = <[u8; 256]>::try_from(bitmap) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::V1ActionsBitmap { bitmap }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.Actions",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_actions_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: Actions<'local>,
) -> JObject<'local> {
    let Ok(maybe_actions) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    let Some(actions) = maybe_actions else {
        // Malformed Java data structure for actions.
        let _ =
            env.throw_new("java/lang/IllegalStateException", "Malformed Actions DE in Java impl");
        return JObject::null();
    };
    use np_ffi_core::serialize::v1::V1SerializeActionsBitmapResult;
    match actions.try_serialize() {
        V1SerializeActionsBitmapResult::Success(generic_de) => {
            let Ok(Some(generic_de)) = Generic::construct(&mut env, generic_de) else {
                // Either a serialization routine yields borked stack data structures,
                // or the JNI has hit an internal error.
                return JObject::null();
            };
            generic_de.0
        }
        V1SerializeActionsBitmapResult::NoActions => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "No actions were present for DE serialization",
            );
            JObject::null()
        }
        V1SerializeActionsBitmapResult::TooManyActions => {
            let _ = TooMuchDataException::throw_new(&mut env);
            JObject::null()
        }
    }
}

/// Rust representation of `class V1DataElement.ContextSyncSeqNum`.
#[repr(transparent)]
pub struct ContextSyncSeqNum<'local>(pub JObject<'local>);

impl<'local> ContextSyncSeqNum<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::ContextSyncSeqNum`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        seq_num: np_ffi_core::v1::ContextSyncSeqNum,
    ) -> jni::errors::Result<Self> {
        let seq_num = seq_num.as_u8() as jint;
        pourover::call_constructor!(env, &CONTEXT_SYNC_SEQ_NUM_CLASS, "(I)V", seq_num).map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::ContextSyncSeqNum`.
    /// Since the Java representation is not range-restricted, this method
    /// returns a wrapped `None` in the case where the value was out of the accepted range.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::ContextSyncSeqNum>> {
        let seq_num =
            pourover::call_method!(env, &CONTEXT_SYNC_SEQ_NUM_CLASS, "getSeqNum", "()I", &self.0)?;
        let Ok(seq_num) = u8::try_from(seq_num) else {
            return Ok(None);
        };
        let np_ffi_core::v1::BuildContextSyncSeqNumResult::Success(seq_num) =
            np_ffi_core::v1::ContextSyncSeqNum::build_from_unsigned_byte(seq_num)
        else {
            return Ok(None);
        };
        Ok(Some(seq_num))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.ContextSyncSeqNum",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_context_sync_seq_num_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: ContextSyncSeqNum<'local>,
) -> JObject<'local> {
    let Ok(maybe_seq_num) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    try_construct_generic_de_from_serialized(
        &mut env,
        maybe_seq_num.and_then(|seq_num| seq_num.try_serialize().ok()),
        "Invalid ContextSyncSeqNum DE",
    )
}

/// Rust representation of `class V1DataElement.DedupHint`.
#[repr(transparent)]
pub struct DedupHint<'local>(pub JObject<'local>);

impl<'local> DedupHint<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::DedupHint`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        dedup_hint: np_ffi_core::v1::DedupHint,
    ) -> jni::errors::Result<Self> {
        let dedup_hint = env.auto_local(env.byte_array_from_slice(&dedup_hint.bytes)?);
        pourover::call_constructor!(env, &DEDUP_HINT_CLASS, "([B)V", dedup_hint.as_ref()).map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::DedupHint`.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::DedupHint>> {
        let bytes = pourover::call_method!(env, &DEDUP_HINT_CLASS, "getBytes", "()[B", &self.0)?;
        let bytes = env.convert_byte_array(bytes)?;
        let Ok(bytes) = <[u8; 8]>::try_from(bytes) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::DedupHint { bytes }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.DedupHint",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_dedup_hint_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: DedupHint<'local>,
) -> JObject<'local> {
    let Ok(maybe_dedup_hint) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    try_construct_generic_de_from_serialized(
        &mut env,
        maybe_dedup_hint.map(|dedup_hint| dedup_hint.serialize()),
        "Invalid Dedup Hint DE",
    )
}

/// Rust representation of `class V1DataElement.DeviceType`.
#[repr(transparent)]
pub struct DeviceType<'local>(pub JObject<'local>);

impl<'local> DeviceType<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::DeviceType`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        device_type: np_ffi_core::v1::DeviceType,
    ) -> jni::errors::Result<Self> {
        let device_type = device_type.device_type as jint;
        pourover::call_constructor!(env, &DEVICE_TYPE_CLASS, "(I)V", device_type).map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::DeviceType`.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::DeviceType>> {
        let device_type =
            pourover::call_method!(env, &DEVICE_TYPE_CLASS, "getValue", "()I", &self.0)?;
        let Ok(device_type) = u8::try_from(device_type) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::DeviceType { device_type }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.DeviceType",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_device_type_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: DeviceType<'local>,
) -> JObject<'local> {
    let Ok(maybe_device_type) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    try_construct_generic_de_from_serialized(
        &mut env,
        maybe_device_type.map(|device_type| device_type.serialize()),
        "Invalid Device Type DE",
    )
}

/// Rust representation of `class V1DataElement.ConnectivityInfo`.
#[repr(transparent)]
pub struct ConnectivityInfo<'local>(pub JObject<'local>);

impl<'local> ConnectivityInfo<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::ConnectivityInfo`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        info: np_ffi_core::v1::ConnectivityInfo,
    ) -> jni::errors::Result<Self> {
        let ble_info = info.get_ble_info();
        let ble_info = if ble_info.is_present() {
            let mac_address = optional_array_to_jni(env, ble_info.mac_address.into_rust())?;
            let psm = optional_array_to_jni(env, ble_info.psm.into_rust())?;
            let device_token = optional_array_to_jni(env, ble_info.device_token.into_rust())?;

            let Ok(gatt_service_identifier) = ble_info.gatt_service_identifier.as_slice() else {
                // This should be unreachable in practice due to the fact that
                // `construct` is only called on data element deserialization, but if
                // we get here, it's equivalent to `InvalidStackDataStructure` in ffi-core
                // or an `IllegalStateException` in Java-land.
                let _ = env.throw_new(
                    "java/lang/IllegalStateException",
                    "ffi-core GATT service identifier array is a malformed stack data-structure",
                );
                return Ok(Self(JObject::null()));
            };
            // Always translate "empty" to null to keep things standardized.
            let gatt_service_identifier = if gatt_service_identifier.is_empty() {
                JObject::null().into()
            } else {
                optional_array_to_jni(env, Some(gatt_service_identifier))?
            };

            pourover::call_constructor!(
                env,
                &BLE_CONNECTIVITY_INFO_CLASS,
                "([B[B[B[B)V",
                &mac_address,
                &gatt_service_identifier,
                &psm,
                &device_token
            )?
        } else {
            JObject::null()
        };

        let wifi_lan_info = info.get_wifi_lan_info();
        let wifi_lan_info = if wifi_lan_info.is_present() {
            let ip = match wifi_lan_info.ip {
                np_ffi_core::v1::OptionalIpAddress::NotPresent => None,
                np_ffi_core::v1::OptionalIpAddress::IPv4(ip) => Some(ip.as_slice().to_vec()),
                np_ffi_core::v1::OptionalIpAddress::IPv6(ip) => Some(ip.as_slice().to_vec()),
            };
            let ip = optional_array_to_jni(env, ip)?;

            let port = optional_array_to_jni(env, wifi_lan_info.port.into_rust())?;
            let bssid = optional_array_to_jni(env, wifi_lan_info.bssid.into_rust())?;

            pourover::call_constructor!(
                env,
                &WIFI_LAN_CONNECTIVITY_INFO_CLASS,
                "([B[B[B)V",
                &ip,
                &port,
                &bssid
            )?
        } else {
            JObject::null()
        };
        pourover::call_constructor!(
            env,
            &CONNECTIVITY_INFO_CLASS,
            "(Lcom/google/android/nearby/presence/rust/V1DataElement$BleConnectivityInfo;\
            Lcom/google/android/nearby/presence/rust/V1DataElement$WifiLanConnectivityInfo;)V",
            &ble_info,
            &wifi_lan_info
        )
        .map(Self)
    }
    /// Attempts to convert a Java instance into a `np_ffi_core::v1::ConnectivityInfo`.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::ConnectivityInfo>> {
        let mut result = np_ffi_core::v1::ConnectivityInfo::default();

        let java_ble_info = pourover::call_method!(
            env,
            &CONNECTIVITY_INFO_CLASS,
            "getBleInfo",
            "()Lcom/google/android/nearby/presence/rust/V1DataElement$BleConnectivityInfo;",
            &self.0
        )?;

        let java_wifi_lan_info = pourover::call_method!(
            env,
            &CONNECTIVITY_INFO_CLASS,
            "getWifiLanInfo",
            "()Lcom/google/android/nearby/presence/rust/V1DataElement$WifiLanConnectivityInfo;",
            &self.0
        )?;

        if !env.is_same_object(&java_ble_info, JObject::null())? {
            let mut ble_info = result.get_ble_info();

            let maybe_mac_address = pourover::call_method!(
                env,
                &BLE_CONNECTIVITY_INFO_CLASS,
                "getMacAddress",
                "()[B",
                &java_ble_info
            )?;
            match optional_fixed_size_array_from_jni(env, maybe_mac_address)? {
                Ok(maybe_mac_address) => {
                    ble_info.mac_address = maybe_mac_address;
                }
                Err(_) => return Ok(None),
            }
            let maybe_gatt_service_identifier = pourover::call_method!(
                env,
                &BLE_CONNECTIVITY_INFO_CLASS,
                "getGattServiceIdentifier",
                "()[B",
                &java_ble_info
            )?;
            match optionally_empty_byte_buffer_from_jni(env, maybe_gatt_service_identifier)? {
                Ok(maybe_gatt_service_identifier) => {
                    ble_info.gatt_service_identifier = maybe_gatt_service_identifier;
                }
                Err(_) => return Ok(None),
            }
            let maybe_psm = pourover::call_method!(
                env,
                &BLE_CONNECTIVITY_INFO_CLASS,
                "getPsm",
                "()[B",
                &java_ble_info
            )?;
            match optional_fixed_size_array_from_jni(env, maybe_psm)? {
                Ok(maybe_psm) => {
                    ble_info.psm = maybe_psm;
                }
                Err(_) => return Ok(None),
            }
            let maybe_device_token = pourover::call_method!(
                env,
                &BLE_CONNECTIVITY_INFO_CLASS,
                "getDeviceToken",
                "()[B",
                &java_ble_info
            )?;
            match optional_fixed_size_array_from_jni(env, maybe_device_token)? {
                Ok(maybe_device_token) => {
                    ble_info.device_token = maybe_device_token;
                }
                Err(_) => return Ok(None),
            }

            result = result.set_ble_info(ble_info);
        }

        if !env.is_same_object(&java_wifi_lan_info, JObject::null())? {
            let mut wifi_lan_info = result.get_wifi_lan_info();

            let maybe_ip = pourover::call_method!(
                env,
                &WIFI_LAN_CONNECTIVITY_INFO_CLASS,
                "getIpAddress",
                "()[B",
                &java_wifi_lan_info
            )?;
            let maybe_ip = optional_byte_array_from_jni(env, maybe_ip)?;
            wifi_lan_info.ip = match maybe_ip {
                Some(ip) => {
                    #[allow(clippy::unwrap_used)]
                    match ip.len() {
                        4 => np_ffi_core::v1::OptionalIpAddress::IPv4(
                            TryInto::<[u8; 4]>::try_into(ip).unwrap().into(),
                        ),
                        16 => np_ffi_core::v1::OptionalIpAddress::IPv6(
                            TryInto::<[u8; 16]>::try_into(ip).unwrap().into(),
                        ),
                        _ => {
                            // Invalid length.
                            return Ok(None);
                        }
                    }
                }
                None => np_ffi_core::v1::OptionalIpAddress::NotPresent,
            };

            let maybe_port = pourover::call_method!(
                env,
                &WIFI_LAN_CONNECTIVITY_INFO_CLASS,
                "getPort",
                "()[B",
                &java_wifi_lan_info
            )?;
            match optional_fixed_size_array_from_jni(env, maybe_port)? {
                Ok(maybe_port) => {
                    wifi_lan_info.port = maybe_port;
                }
                Err(_) => return Ok(None),
            }

            let maybe_bssid = pourover::call_method!(
                env,
                &WIFI_LAN_CONNECTIVITY_INFO_CLASS,
                "getBssid",
                "()[B",
                &java_wifi_lan_info
            )?;
            match optional_fixed_size_array_from_jni(env, maybe_bssid)? {
                Ok(maybe_bssid) => {
                    wifi_lan_info.bssid = maybe_bssid;
                }
                Err(_) => return Ok(None),
            }
            result = result.set_wifi_lan_info(wifi_lan_info);
        }
        Ok(Some(result))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.ConnectivityInfo",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_connectivity_info_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: ConnectivityInfo<'local>,
) -> JObject<'local> {
    let Ok(maybe_connectivity_info) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    try_construct_generic_de_from_serialized(
        &mut env,
        maybe_connectivity_info
            .and_then(|connectivity_info| connectivity_info.try_serialize().ok()),
        "Attempt to Serialize Invalid Connectivity Info DE",
    )
}

/// Rust representation of `class V1DataElement.Capabilities`.
#[repr(transparent)]
pub struct Capabilities<'local>(pub JObject<'local>);

impl<'local> Capabilities<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::Capabilities`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        capabilities: np_ffi_core::v1::Capabilities,
    ) -> jni::errors::Result<Self> {
        let capabilities = capabilities.capabilities as jbyte;
        pourover::call_constructor!(env, &CAPABILITIES_CLASS, "(B)V", capabilities).map(Self)
    }

    /// Attempts to convert a Java instance into a `np_ffi_core::v1::Capabilities`.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::Capabilities>> {
        let bitmap = pourover::call_method!(env, &CAPABILITIES_CLASS, "getValue", "()B", &self.0)?;
        let Ok(capabilities) = u8::try_from(bitmap) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::Capabilities { capabilities }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.Capabilities",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_capabilities_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: Capabilities<'local>,
) -> JObject<'local> {
    let Ok(maybe_capabilities) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    let Some(capabilities) = maybe_capabilities else {
        // Malformed Java data structure for capabilities.
        let _ = env
            .throw_new("java/lang/IllegalStateException", "Malformed Capabilities DE in Java impl");
        return JObject::null();
    };
    use np_ffi_core::serialize::v1::SerializeCapabilitiesBitmapResult;
    match capabilities.try_serialize() {
        SerializeCapabilitiesBitmapResult::Success(generic_de) => {
            let Ok(Some(generic_de)) = Generic::construct(&mut env, generic_de) else {
                // Either a serialization routine yields borked stack data structures,
                // or the JNI has hit an internal error.
                return JObject::null();
            };
            generic_de.0
        }
        SerializeCapabilitiesBitmapResult::ReservedBitsSet => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Reserved bits set in Capabilities DE serialization",
            );
            JObject::null()
        }
    }
}

/// Rust representation of `class V1DataElement.Requirements`.
#[repr(transparent)]
pub struct Requirements<'local>(pub JObject<'local>);

impl<'local> Requirements<'local> {
    /// Create a new Java instance from the given `np_ffi_core::v1::Requirements`.
    pub fn construct(
        env: &mut JNIEnv<'local>,
        requirements: np_ffi_core::v1::Requirements,
    ) -> jni::errors::Result<Self> {
        let requirements = requirements.requirements as jbyte;
        pourover::call_constructor!(env, &REQUIREMENTS_CLASS, "(B)V", requirements).map(Self)
    }

    /// Attempts to convert a Java instance into a `np_ffi_core::v1::Requirements`.
    pub fn as_core<'env>(
        &self,
        env: &mut JNIEnv<'env>,
    ) -> jni::errors::Result<Option<np_ffi_core::v1::Requirements>> {
        let bitmap = pourover::call_method!(env, &REQUIREMENTS_CLASS, "getValue", "()B", &self.0)?;
        let Ok(requirements) = u8::try_from(bitmap) else {
            return Ok(None);
        };
        Ok(Some(np_ffi_core::v1::Requirements { requirements }))
    }
}

#[jni_method(
    package = "com.google.android.nearby.presence.rust",
    class = "V1DataElement.Requirements",
    method_name = "nativeTrySerialize"
)]
extern "system" fn try_serialize_requirements_de<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    this: Requirements<'local>,
) -> JObject<'local> {
    let Ok(maybe_requirements) = this.as_core(&mut env) else {
        // Unrecoverable JNI error.
        return JObject::null();
    };
    let Some(requirements) = maybe_requirements else {
        // Malformed Java data structure for requirements.
        let _ = env
            .throw_new("java/lang/IllegalStateException", "Malformed Requirements DE in Java impl");
        return JObject::null();
    };
    use np_ffi_core::serialize::v1::SerializeRequirementsBitmapResult;
    match requirements.try_serialize() {
        SerializeRequirementsBitmapResult::Success(generic_de) => {
            let Ok(Some(generic_de)) = Generic::construct(&mut env, generic_de) else {
                // Either a serialization routine yields borked stack data structures,
                // or the JNI has hit an internal error.
                return JObject::null();
            };
            generic_de.0
        }
        SerializeRequirementsBitmapResult::ReservedBitsSet => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Reserved bits set in Requirements DE serialization",
            );
            JObject::null()
        }
    }
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

/// Error raised when one of the from-JNI `byte[]` conversion method
/// gets a non-`null` result, but the length doesn't match the length
/// of the expected target ffi-core array.
struct InvalidArrayLength;

/// Reads a `byte[]` into an ffi-core `ByteBuffer`, where `null` corresponds to empty.
fn optionally_empty_byte_buffer_from_jni<'local, const N: usize>(
    env: &mut JNIEnv<'local>,
    array: impl AsRef<JByteArray<'local>>,
) -> jni::errors::Result<Result<np_ffi_core::common::ByteBuffer<N>, InvalidArrayLength>> {
    let maybe_array = optional_byte_array_from_jni(env, array)?;
    match maybe_array {
        Some(array) => {
            if array.len() <= N {
                let mut result = tinyvec::ArrayVec::new();
                result.extend_from_slice(&array);
                Ok(Ok(np_ffi_core::common::ByteBuffer::from_array_vec(result)))
            } else {
                Ok(Err(InvalidArrayLength))
            }
        }
        None => Ok(Ok(np_ffi_core::common::ByteBuffer::default())),
    }
}

fn optional_fixed_size_array_from_jni<'local, const N: usize>(
    env: &mut JNIEnv<'local>,
    array: impl AsRef<JByteArray<'local>>,
) -> jni::errors::Result<Result<np_ffi_core::common::OptionalFixedSizeArray<N>, InvalidArrayLength>>
{
    let maybe_array = optional_byte_array_from_jni(env, array)?;
    match maybe_array {
        Some(array) => match TryInto::<[u8; N]>::try_into(array) {
            Ok(array) => Ok(Ok(Some(array).into())),
            Err(_) => Ok(Err(InvalidArrayLength)),
        },
        None => Ok(Ok(np_ffi_core::common::OptionalFixedSizeArray::default())),
    }
}

fn optional_array_to_jni<'local, A: AsRef<[u8]>>(
    env: &mut JNIEnv<'local>,
    maybe_array: Option<A>,
) -> jni::errors::Result<JByteArray<'local>> {
    match maybe_array {
        Some(array) => env.byte_array_from_slice(array.as_ref()),
        None => Ok(JObject::null().into()),
    }
}
