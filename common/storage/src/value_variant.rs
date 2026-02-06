// Copyright (C) 2023 The Android Open Source Project
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

#[cfg(any(target_os = "macos", target_os = "ios"))]
use objc2::{rc::Retained, AllocAnyThread};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use objc2_foundation::{NSNumber, NSString};

pub(crate) mod values {
    pub trait Sealed {}
    pub struct Token;
}

/// Encapsulates a key type for the key-value bag.
pub trait Key {
    /// The name of the key to store the value in.
    const KEY: &'static str;
    /// The value type.
    type Value: ValueType;
}

/// Conversion errors when converting from `ValueType` to the underlying representation that the store uses.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// Unsupported conversion of a serde_json value.
    #[error("Unsupported conversion of `{0}`!")]
    UnsupportedConversion(String),
}

/// Marker trait for allowed value types.
pub trait ValueType: values::Sealed + Sized + Clone + 'static + Send + Sync {
    /// Converts this `ValueType` to a `serde_json::Value`.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value;
    /// Converts a `serde_json::Value` to this `ValueType`.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError>;
    /// Converts an `NSObject` to this `ValueType`.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError>;
    /// Converts this `ValueType` to an `NSObject`.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject>;
}

impl values::Sealed for u8 {}
impl values::Sealed for u32 {}
impl values::Sealed for f32 {}
impl values::Sealed for u64 {}
impl values::Sealed for u128 {}
impl values::Sealed for bool {}
impl values::Sealed for String {}

impl ValueType for u8 {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::Number(self.into())
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value
            .as_u64()
            .map(|u| u as u8)
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value
            .downcast_ref::<NSNumber>()
            .map(|n| n.as_u8())
            .ok_or(ConversionError::UnsupportedConversion(format!("{value:?}")))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        let allocated = NSNumber::alloc();
        NSNumber::initWithUnsignedChar(allocated, self)
            .into_super()
            .into_super()
    }
}

impl ValueType for u32 {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::Number(self.into())
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value
            .as_u64()
            .map(|u| u as u32)
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value
            .downcast_ref::<NSNumber>()
            .map(|n| n.as_u32())
            .ok_or(ConversionError::UnsupportedConversion(format!("{value:?}")))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        let allocated = NSNumber::alloc();
        NSNumber::initWithUnsignedInt(allocated, self)
            .into_super()
            .into_super()
    }
}

impl ValueType for f32 {
    // f32 -> f64 conversions should always succeed.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    #[allow(clippy::expect_used)]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::Number(
            serde_json::Number::from_f64(self as f64)
                .expect("Should have converted valid f32 to f64!"),
        )
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value
            .as_f64()
            .map(|f| f as f32)
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value
            .downcast_ref::<NSNumber>()
            .map(|n| n.as_f32())
            .ok_or(ConversionError::UnsupportedConversion(format!("{value:?}")))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        let allocated = NSNumber::alloc();
        NSNumber::initWithFloat(allocated, self)
            .into_super()
            .into_super()
    }
}

impl ValueType for u64 {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::Number(self.into())
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value.as_u64().ok_or(ConversionError::UnsupportedConversion(
            format!("{value:?}",),
        ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value.downcast_ref::<NSNumber>().map(|n| n.as_u64()).ok_or(
            ConversionError::UnsupportedConversion(format!("{value:?}",)),
        )
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        let allocated = NSNumber::alloc();
        NSNumber::initWithUnsignedLongLong(allocated, self)
            .into_super()
            .into_super()
    }
}

impl ValueType for u128 {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::String(self.to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value.as_str().and_then(|s| s.parse::<u128>().ok()).ok_or(
            ConversionError::UnsupportedConversion(format!("{value:?}",)),
        )
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[allow(unsafe_code)]
    // SAFETY: The Objective-C runtime returns a UTF-8 string. `from_raw_parts` builds a slice from
    // a valid pointer given by the Objective-C runtime.
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value
            .downcast_ref::<NSString>()
            .and_then(|n| unsafe {
                let slice = std::slice::from_raw_parts(n.UTF8String() as *const u8, n.len());
                std::str::from_utf8(slice)
                    .ok()
                    .and_then(|s| s.parse::<u128>().ok())
            })
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        NSString::from_str(&self.to_string()).into_super()
    }
}

impl ValueType for bool {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::Bool(self)
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value
            .as_bool()
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value.downcast_ref::<NSNumber>().map(|n| n.as_bool()).ok_or(
            ConversionError::UnsupportedConversion(format!("{value:?}",)),
        )
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        let allocated = NSNumber::alloc();
        NSNumber::initWithBool(allocated, self)
            .into_super()
            .into_super()
    }
}

impl ValueType for String {
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn to_value(self, _: values::Token) -> serde_json::Value {
        serde_json::Value::String(self)
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn try_from_value(value: serde_json::Value, _: values::Token) -> Result<Self, ConversionError> {
        value
            .as_str()
            .map(|s| s.to_string())
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[allow(unsafe_code)]
    // SAFETY: The Objective-C runtime returns a UTF-8 string. `from_raw_parts` builds a slice from
    // a valid pointer given by the Objective-C runtime.
    fn try_from_nsobject(
        value: &objc2_foundation::NSObject,
        _: values::Token,
    ) -> Result<Self, ConversionError> {
        value
            .downcast_ref::<NSString>()
            .and_then(|n| unsafe {
                let slice = std::slice::from_raw_parts(n.UTF8String() as *const u8, n.len());
                String::from_utf8(slice.to_vec()).ok()
            })
            .ok_or(ConversionError::UnsupportedConversion(
                format!("{value:?}",),
            ))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn to_nsobject(self, _: values::Token) -> Retained<objc2_foundation::NSObject> {
        NSString::from_str(&self).into_super()
    }
}
