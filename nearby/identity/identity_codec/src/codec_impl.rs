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
use crate::{
    api::Codec,
    encode::{encode_control, encode_dct_ios},
    errors::{DecodingWithVisibilityError, EncodingWithVisibilityError},
    types::{AdvertisementServiceData, ContentTypeHint, DecodedResult, EncodingRequest},
};
use async_trait::async_trait;
use crypto_provider::{CryptoProvider, CryptoRng};
use identity_common::definitions::{VisibilityLevel, VisibilityScope};
use identity_manager::{
    api::IdentityManager,
    errors::IdentityManagerError,
    types::{AuthToken, BroadcastSessionId, StableDeviceId},
};
use std::marker::PhantomData;
use std::sync::Arc;

pub(crate) struct CodecImpl<C: CryptoProvider> {
    pub(crate) identity_manager: Arc<dyn IdentityManager>,
    crypto_provider: PhantomData<C>,
}

impl<C: CryptoProvider + Send + Sync + 'static> CodecImpl<C> {
    pub(crate) fn new(identity_manager: Arc<dyn IdentityManager>) -> Self {
        Self { identity_manager, crypto_provider: PhantomData }
    }
}

#[cfg(feature = "encrypted_identity")]
#[async_trait]
impl<C: CryptoProvider + Send + Sync + 'static> Codec for CodecImpl<C>
where
    C::CryptoRng: CryptoRng,
{
    fn encode_with_visibility(
        &self,
        request: &EncodingRequest,
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
        match request {
            EncodingRequest::V0 { visibility, data_elements } => match visibility {
                VisibilityLevel::Public => self.encode_v0_plaintext(data_elements),
                _ => self.encode_v0_encrypted(visibility, data_elements),
            },
            EncodingRequest::V1 { data_elements } => self.encode_v1(data_elements),
            EncodingRequest::DctIos { data_elements } => encode_dct_ios(data_elements),
            EncodingRequest::Control { short_format, data_elements } => {
                encode_control::<C>(*short_format, data_elements)
            }
        }
    }

    fn decode_with_visibility(
        &self,
        data: &[u8],
        content_type_hint: ContentTypeHint,
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        match content_type_hint {
            ContentTypeHint::Up => self.decode_up(data),
            _ => unimplemented!(),
        }
    }

    fn is_decoded_device_valid(&self, _auth_token: AuthToken) -> bool {
        unimplemented!()
    }

    fn is_the_same_device(
        &self,
        _stable_id: StableDeviceId,
        _broadcast_session_id: BroadcastSessionId,
    ) -> bool {
        unimplemented!()
    }

    async fn broadcast_credentials_changed(&self) -> Result<VisibilityScope, IdentityManagerError> {
        self.identity_manager.broadcast_credentials_changed().await
    }

    async fn discovery_credentials_changed(&self) -> Result<(), IdentityManagerError> {
        self.identity_manager.discovery_credentials_changed().await
    }

    fn rotate_broadcast_session_id(&self) {
        self.identity_manager.rotate_broadcast_session_id()
    }
}

#[cfg(not(feature = "encrypted_identity"))]
#[async_trait]
impl<C: CryptoProvider + Send + Sync + 'static> Codec for CodecImpl<C>
where
    C::CryptoRng: CryptoRng,
{
    fn encode_with_visibility(
        &self,
        request: &EncodingRequest,
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
        match request {
            EncodingRequest::V0 { visibility, data_elements } => match visibility {
                VisibilityLevel::Public => self.encode_v0_plaintext(data_elements),
                _ => Err(EncodingWithVisibilityError::UnsupportedApiRequest),
            },
            EncodingRequest::V1 { data_elements } => self.encode_v1_public_only(data_elements),
            EncodingRequest::DctIos { data_elements } => encode_dct_ios(data_elements),
            EncodingRequest::Control { short_format, data_elements } => {
                encode_control::<C>(*short_format, data_elements)
            }
        }
    }

    fn decode_with_visibility(
        &self,
        data: &[u8],
        content_type_hint: ContentTypeHint,
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        match content_type_hint {
            ContentTypeHint::Up => Self::public_decode_up(data),
            _ => unimplemented!(),
        }
    }

    fn is_decoded_device_valid(&self, _auth_token: AuthToken) -> bool {
        unimplemented!()
    }

    fn is_the_same_device(
        &self,
        _stable_id: StableDeviceId,
        _broadcast_session_id: BroadcastSessionId,
    ) -> bool {
        unimplemented!()
    }

    async fn broadcast_credentials_changed(&self) -> Result<VisibilityScope, IdentityManagerError> {
        self.identity_manager.broadcast_credentials_changed().await
    }

    async fn discovery_credentials_changed(&self) -> Result<(), IdentityManagerError> {
        self.identity_manager.discovery_credentials_changed().await
    }

    fn rotate_broadcast_session_id(&self) {
        self.identity_manager.rotate_broadcast_session_id()
    }
}
