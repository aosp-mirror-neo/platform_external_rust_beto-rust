// Copyright 2025 Google LLC
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

//! Common types related to identities.

/// Defines the visibility scope of a broadcast.
///
/// This visibility scope can have an optional provider specified, indicating the scope should be
/// further constrained to only use credentials of a particular identity provider.
/// However, under most circumstances, the provider should be left unspecified, so the entire
/// identity pool with all providers can be leveraged for encoding and decoding.
/// Typically, the provider API will only be set by 3P apps which wish to restrict the advertising
/// and discovery of private data. For more detail, see go/d2di-private-data-elements.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum VisibilityScope {
    /// Unknown visibility scope.
    Unknown,
    /// Limited to devices within a private scope.
    SelfScope {
        /// Optional external provider name.
        provider: Option<String>,
    },
    /// Visible to family members.
    Family {
        /// Optional external provider name.
        provider: Option<String>,
    },
    /// Visible to the user's contacts.
    Contact {
        /// Optional external provider name.
        provider: Option<String>,
    },
    /// Visible to all nearby devices.
    Public,
}

/// The visibility level of a broadcast.
// LINT.IfChange(visibility_level)
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum VisibilityLevel {
    /// Unknown visibility level.
    Unknown,
    /// Visible to self-owned devices.
    SelfScope,
    /// Visible to family members.
    Family,
    /// Visible to the user's contacts.
    Contact,
    /// Visible to all nearby devices.
    Public,
}
// LINT.ThenChange(nearby/identity/credential_proto/proto/credential.proto:visibility_level)

impl VisibilityScope {
    /// Creates a visibility scope from level and provider.
    pub fn new(level: VisibilityLevel, provider: Option<&str>) -> Self {
        let provider = provider.map(ToString::to_string);
        match level {
            VisibilityLevel::SelfScope => VisibilityScope::SelfScope { provider },
            VisibilityLevel::Family => VisibilityScope::Family { provider },
            VisibilityLevel::Contact => VisibilityScope::Contact { provider },
            VisibilityLevel::Public => VisibilityScope::Public,
            VisibilityLevel::Unknown => VisibilityScope::Unknown,
        }
    }

    /// Returns the general visibility level of the scope.
    pub fn level(&self) -> VisibilityLevel {
        match self {
            VisibilityScope::SelfScope { .. } => VisibilityLevel::SelfScope,
            VisibilityScope::Family { .. } => VisibilityLevel::Family,
            VisibilityScope::Contact { .. } => VisibilityLevel::Contact,
            VisibilityScope::Public => VisibilityLevel::Public,
            VisibilityScope::Unknown => VisibilityLevel::Unknown,
        }
    }

    /// Gets the optional identity provider name associated with the visibility scope.
    pub fn provider(&self) -> Option<&str> {
        match self {
            VisibilityScope::SelfScope { provider } => provider.as_deref(),
            VisibilityScope::Family { provider } => provider.as_deref(),
            VisibilityScope::Contact { provider } => provider.as_deref(),
            VisibilityScope::Public => None,
            VisibilityScope::Unknown => None,
        }
    }
}

/// Config for registering identity providers.
#[derive(Debug, Clone)]
pub struct IdentityProviderConfig {
    /// Provider name
    pub provider_name: ProviderName,
    /// Visibility levels
    pub visibility_levels: Vec<VisibilityLevel>,
}

/// Identity provider name.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProviderName(pub String);

impl Default for ProviderName {
    fn default() -> Self {
        ProviderName("com.google".to_string())
    }
}

/// Local device information.
#[derive(Debug, Clone, Default)]
pub struct DeviceInfo {
    /// Local device name.
    pub device_name: String,
}

/// Type of the broadcast device.
pub type DeviceType = up_adv::shared_data::DeviceType;
