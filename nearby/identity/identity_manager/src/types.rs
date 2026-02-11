// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
//
// you may not use this file except in compliance with the License.
//
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
//
// distributed under the License is distributed on an "AS IS" BASIS,
//
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
// See the License for the specific language governing permissions and
//
// limitations under the License.
//
//

use crate::{errors::IdentityManagerError, identity_storage::DEVICE_ID_LENGTH};

/// The stable device ID is a property used to identify a device across different broadcast sessions.
/// It is distributed *only* to SELF scope peer devices (i.e. same-account devices), and is packaged
/// in the encrypted metadata of a SELF scope credential.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct StableDeviceId(pub [u8; DEVICE_ID_LENGTH]);

impl StableDeviceId {
    /// Creates the ID from a 16-byte array.
    pub fn new(id: [u8; DEVICE_ID_LENGTH]) -> Self {
        Self(id)
    }
}

impl TryFrom<&[u8]> for StableDeviceId {
    type Error = IdentityManagerError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let len = value.len();
        let bytes: [u8; DEVICE_ID_LENGTH] =
            value.try_into().map_err(|_| IdentityManagerError::InvalidDeviceIdLength(len))?;
        Ok(StableDeviceId::new(bytes))
    }
}

/// Size of broadcast session ID for public beto core advertisements.
pub const BROADCAST_SESSION_ID_SIZE: usize =
    up_adv::extended::data_elements::BROADCAST_SESSION_ID_LENGTH;
const DEDUPE_SALT_SIZE: usize = 2;

/// Byte array used for device deduplication / broadcast session identification.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BroadcastSessionId([u8; BROADCAST_SESSION_ID_SIZE]);

impl BroadcastSessionId {
    /// Creates the ID from a byte array.
    pub fn new(bytes: [u8; BROADCAST_SESSION_ID_SIZE]) -> Self {
        Self(bytes)
    }

    /// Salt part of the session ID.
    #[allow(unused)]
    pub(crate) fn salt(&self) -> &[u8] {
        &self.0[0..DEDUPE_SALT_SIZE]
    }

    /// Hash part of the session ID.
    #[allow(unused)]
    pub(crate) fn hash(&self) -> &[u8] {
        &self.0[DEDUPE_SALT_SIZE..BROADCAST_SESSION_ID_SIZE]
    }

    /// Broadcast session ID bytes.
    pub fn as_bytes(&self) -> &[u8; BROADCAST_SESSION_ID_SIZE] {
        &self.0
    }
}

/// A token representing the device's identity within a certain visibility scope.
///
/// Used to sign and verify signatures during a connection authentication handshake.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AuthToken(u64);

impl AuthToken {
    /// Creates an identity token.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}
