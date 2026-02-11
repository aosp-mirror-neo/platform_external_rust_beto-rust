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

use crate::{
    errors::{DecodingWithVisibilityError, EncodingWithVisibilityError},
    types::{AdvertisementServiceData, ContentTypeHint, DecodedResult, EncodingRequest},
};
use async_trait::async_trait;
use identity_common::definitions::VisibilityScope;
use identity_manager::{
    errors::IdentityManagerError,
    types::{AuthToken, BroadcastSessionId, StableDeviceId},
};
#[cfg(feature = "test-utils")]
use mockall::automock;

/// A Codec trait for encoding and decoding Unified Protocol advertisements.
/// This trait supports the following advertisement formats:
/// - V0: Unified Protocol short format (legacy) for BLE 4.2 advertisements.
/// - V1: Unified Protocol extended format with identity and visibility scopes.
#[async_trait]
#[cfg_attr(feature = "test-utils", automock)]
pub trait Codec: Send + Sync {
    /// Encode a set of data elements:
    ///     @param encoding_request is an enum containing the visibility scope(s) and data elements requested by a client.
    ///     @return advertisement service data.
    fn encode_with_visibility(
        &self,
        encoding_request: &EncodingRequest,
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError>;

    /// Decode raw bytes for an incoming discovery event.
    ///     @param data is the raw byte slice.
    ///     @param content_type_hint is a type hint to help `Codec` find the right decoding algorithm.
    ///     @return a vector of `DiscoveredDeviceInfo`` which contains device identity, parsed metadata, and parsed data elements.
    fn decode_with_visibility(
        &self,
        data: &[u8],
        content_type_hint: ContentTypeHint,
    ) -> Result<DecodedResult, DecodingWithVisibilityError>;

    /// Check if the discovered device associated with the given auth token is still valid
    /// after discovery credentials change.
    fn is_decoded_device_valid(&self, auth_token: AuthToken) -> bool;

    /// Checks if a broadcast session ID matches the device identity of a stable device ID.
    fn is_the_same_device(
        &self,
        stable_id: StableDeviceId,
        broadcast_session_id: BroadcastSessionId,
    ) -> bool;

    /// Broadcast credential change listener.
    async fn broadcast_credentials_changed(&self) -> Result<VisibilityScope, IdentityManagerError>;

    /// Peer discovery credential change listener.
    async fn discovery_credentials_changed(&self) -> Result<(), IdentityManagerError>;

    /// Rotates the broadcast session ID used in public advertisements.
    fn rotate_broadcast_session_id(&self);
}
