// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[allow(clippy::unwrap_used, missing_docs)]
#[cfg(all(not(feature = "blaze"), not(feature = "soong")))]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto/mod.rs"));
}

#[cfg(any(feature = "blaze", feature = "soong"))]
mod precompiled;

#[allow(clippy::unwrap_used, missing_docs)]
#[cfg(any(feature = "blaze", feature = "soong"))]
pub mod proto {
    pub use crate::precompiled::*;
}

use crate::proto::credential::{VisibilityLevel, VisibilityScope as ProtoVisibilityScope};
use identity_common::definitions::{DeviceType, VisibilityScope};

impl From<VisibilityScope> for ProtoVisibilityScope {
    fn from(visibility: VisibilityScope) -> Self {
        let level: VisibilityLevel;
        let mut provider_name = "".to_string();
        match visibility {
            VisibilityScope::Public => {
                level = VisibilityLevel::VISIBILITY_LEVEL_PUBLIC;
            }
            VisibilityScope::SelfScope { provider } => {
                if let Some(name) = provider {
                    provider_name = name;
                }
                level = VisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP;
            }
            VisibilityScope::Family { provider } => {
                if let Some(name) = provider {
                    provider_name = name;
                }
                level = VisibilityLevel::VISIBILITY_LEVEL_FAMILY_GROUP;
            }
            VisibilityScope::Contact { provider } => {
                if let Some(name) = provider {
                    provider_name = name;
                }
                level = VisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP;
            }
            VisibilityScope::Unknown => {
                level = VisibilityLevel::VISIBILITY_LEVEL_UNSPECIFIED;
            }
        };

        let mut scope = ProtoVisibilityScope::new();
        scope.level = level.into();
        scope.provider_name = provider_name;
        scope
    }
}

impl From<ProtoVisibilityScope> for VisibilityScope {
    fn from(scope: ProtoVisibilityScope) -> Self {
        match scope.level.enum_value_or_default() {
            VisibilityLevel::VISIBILITY_LEVEL_PUBLIC => VisibilityScope::Public,
            VisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP => VisibilityScope::SelfScope {
                provider: if scope.provider_name.is_empty() {
                    None
                } else {
                    Some(scope.provider_name)
                },
            },
            VisibilityLevel::VISIBILITY_LEVEL_FAMILY_GROUP => VisibilityScope::Family {
                provider: if scope.provider_name.is_empty() {
                    None
                } else {
                    Some(scope.provider_name)
                },
            },
            VisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP => VisibilityScope::Contact {
                provider: if scope.provider_name.is_empty() {
                    None
                } else {
                    Some(scope.provider_name)
                },
            },
            VisibilityLevel::VISIBILITY_LEVEL_UNSPECIFIED => VisibilityScope::Unknown,
        }
    }
}

impl From<proto::metadata::DeviceType> for DeviceType {
    fn from(value: proto::metadata::DeviceType) -> Self {
        match value {
            proto::metadata::DeviceType::DEVICE_TYPE_UNKNOWN => DeviceType::Unknown,
            proto::metadata::DeviceType::DEVICE_TYPE_PHONE => DeviceType::Phone,
            proto::metadata::DeviceType::DEVICE_TYPE_TABLET => DeviceType::Tablet,
            proto::metadata::DeviceType::DEVICE_TYPE_DISPLAY => DeviceType::Display,
            proto::metadata::DeviceType::DEVICE_TYPE_LAPTOP => DeviceType::Laptop,
            proto::metadata::DeviceType::DEVICE_TYPE_TV => DeviceType::TV,
            proto::metadata::DeviceType::DEVICE_TYPE_WATCH => DeviceType::Watch,
            proto::metadata::DeviceType::DEVICE_TYPE_CHROMEOS => DeviceType::ChromeOS,
            proto::metadata::DeviceType::DEVICE_TYPE_FOLDABLE => DeviceType::Foldable,
            proto::metadata::DeviceType::DEVICE_TYPE_AUTOMOTIVE => DeviceType::Automotive,
            proto::metadata::DeviceType::DEVICE_TYPE_SPEAKER => DeviceType::Speaker,
        }
    }
}

impl From<DeviceType> for proto::metadata::DeviceType {
    fn from(value: DeviceType) -> Self {
        match value {
            DeviceType::Unknown => proto::metadata::DeviceType::DEVICE_TYPE_UNKNOWN,
            DeviceType::Phone => proto::metadata::DeviceType::DEVICE_TYPE_PHONE,
            DeviceType::Tablet => proto::metadata::DeviceType::DEVICE_TYPE_TABLET,
            DeviceType::Display => proto::metadata::DeviceType::DEVICE_TYPE_DISPLAY,
            DeviceType::Laptop => proto::metadata::DeviceType::DEVICE_TYPE_LAPTOP,
            DeviceType::TV => proto::metadata::DeviceType::DEVICE_TYPE_TV,
            DeviceType::Watch => proto::metadata::DeviceType::DEVICE_TYPE_WATCH,
            DeviceType::ChromeOS => proto::metadata::DeviceType::DEVICE_TYPE_CHROMEOS,
            DeviceType::Foldable => proto::metadata::DeviceType::DEVICE_TYPE_FOLDABLE,
            DeviceType::Automotive => proto::metadata::DeviceType::DEVICE_TYPE_AUTOMOTIVE,
            DeviceType::Speaker => proto::metadata::DeviceType::DEVICE_TYPE_SPEAKER,
        }
    }
}
