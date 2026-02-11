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
use crate::codec_impl::CodecImpl;
use crate::{
    errors::DecodingWithVisibilityError,
    types::{DecodedDeviceInfo, DecodedResult, DeviceUniqueId},
};
use common::data_elements::{CoreDataElement, ServiceId};
use crypto_provider::CryptoProvider;
use identity_common::definitions::{DeviceType, VisibilityScope};
use identity_manager::types::BroadcastSessionId;
use std::net::IpAddr;
use up_adv::{
    buffer::VecBuffer,
    credential::{book::PrecalculatedOwnedCredentialBookWithId, matched::MatchedCredentialWithId},
    default_vec_buffer, deserialization_arena, deserialize_advertisement,
    extended::{
        data_elements::{
            BleConnectivityInfo, BroadcastSessionIdDataElement, CapabilitiesDataElement,
            CastIdDataElement, ConnectivityInfoDataElement, ContextSyncSeqNumDataElement,
            DeserializedActionsDE, DiscoveryTypeDataElement, MediaDeduplicationIdDataElement,
            RequirementsDataElement, WifiLanConnectivityInfo,
        },
        de_type::HasDEType,
        deserialize::{
            data_element::{DataElementParsingIterator, DeserializedDataElement as _},
            Section as _, V1AdvertisementContents, V1DeserializedSection,
        },
    },
    legacy::{
        deserialize::{DeIterator, DeserializedDataElement, UnencryptedAdvContents},
        PacketFlavor, V0AdvertisementContents,
    },
    shared_data::{ContextSyncSeqNum, DeviceCapability, TxPower},
    DeserializedAdvertisement,
};
#[cfg(feature = "encrypted_identity")]
use up_adv::{
    credential::matched::{MatchedMetadataDecryptionError, WithMatchedCredential},
    extended::deserialize::DecryptedSection,
    legacy::deserialize::DecryptedAdvContents,
};

enum DecodedDataElements {
    UP {
        device_name: String,
        is_name_truncated: bool,
        device_type: DeviceType,
        broadcast_session_id: Option<BroadcastSessionId>,
        data_elements: Vec<CoreDataElement>,
    },
    Control {
        data_elements: Vec<CoreDataElement>,
    },
}

#[cfg(not(feature = "encrypted_identity"))]
impl<C: CryptoProvider + Send + Sync + 'static> CodecImpl<C> {
    pub(super) fn public_decode_up(
        data: &[u8],
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        use up_adv::credential::book::CredentialBookBuilder;
        let credential_book = std::sync::Arc::new(
            CredentialBookBuilder::build_precalculated_owned_book_with_id::<C>([], []),
        );
        let mut buffer = default_vec_buffer();
        let arena = deserialization_arena!(&mut buffer);
        match deserialize_advertisement::<
            PrecalculatedOwnedCredentialBookWithId<
                up_adv::credential::matched::MatchedCredentialWithId<Vec<u8>>,
            >,
            VecBuffer,
            C,
        >(arena, data, &*credential_book)?
        {
            DeserializedAdvertisement::V0(v0) => match v0 {
                V0AdvertisementContents::Plaintext(adv_contents) => {
                    decode_v0_plaintext(adv_contents)
                }
                _ => {
                    log::info!("No matching credentials, failed to decode V0 advertisement.");
                    Ok(DecodedResult::NotDecoded)
                }
            },
            DeserializedAdvertisement::V1(adv_contents) => {
                Self::decode_v1_public_only(adv_contents)
            }
        }
    }

    fn decode_v1_public_only(
        adv_contents: V1AdvertisementContents<MatchedCredentialWithId<Vec<u8>>>,
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        let mut decoded_info_list = Vec::new();
        let mut broadcast_session_id: Option<BroadcastSessionId> = None;
        if adv_contents.sections().size_hint().0 != 1 {
            return Err(DecodingWithVisibilityError::InvalidPublicV1Frame);
        }

        #[allow(clippy::expect_used)]
        if let V1DeserializedSection::Plaintext(plaintext_section) =
            adv_contents.sections().next().expect("Sections should contain one public section.")
        {
            match decode_v1_data_elements(plaintext_section.iter_data_elements())? {
                DecodedDataElements::UP {
                    device_name,
                    is_name_truncated,
                    device_type,
                    broadcast_session_id: de_session_id,
                    data_elements,
                } => {
                    let de_session_id = de_session_id
                        .ok_or(DecodingWithVisibilityError::MissingBroadcastSessionId)?;
                    let decoded_info = DecodedDeviceInfo {
                        data_elements,
                        device_name,
                        is_name_truncated,
                        device_image_url: None,
                        device_type,
                        auth_token: None,
                        visibility_scope: VisibilityScope::Public,
                    };
                    decoded_info_list.push(decoded_info);
                    if broadcast_session_id.is_none() {
                        broadcast_session_id = Some(de_session_id);
                    }
                }
                DecodedDataElements::Control { data_elements } => {
                    if adv_contents.sections().size_hint().0 > 1 {
                        return Err(DecodingWithVisibilityError::InvalidPublicV1Frame);
                    }
                    return Ok(DecodedResult::Control { data_elements });
                }
            }
        };
        // }
        let device_unique_id = if let Some(id) = broadcast_session_id {
            DeviceUniqueId::Public(id)
        } else {
            return Err(DecodingWithVisibilityError::MissingBroadcastSessionId);
        };
        Ok(DecodedResult::V1 {
            device_unique_id,
            decoded_info_list,
            invalid_sections_count: adv_contents.invalid_sections_count(),
        })
    }
}

#[cfg(feature = "encrypted_identity")]
impl<C: CryptoProvider + Send + Sync + 'static> CodecImpl<C> {
    pub(super) fn decode_up(
        &self,
        data: &[u8],
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        use identity_manager::credentials::lazy_book::LazyDiscoveryCredentialBook;
        use identity_manager::credentials::EncryptedMetadata;
        use up_adv::credential::book::ChainedCredentialBook;

        let private_book = self.identity_manager.get_private_group_discovery_credential_book()?;
        let frequent_book =
            self.identity_manager.get_frequent_contact_discovery_credential_book()?;
        let on_demand_book =
            self.identity_manager.get_on_demand_contact_discovery_credential_book()?;
        let chains = ChainedCredentialBook::new(&*private_book, &*frequent_book, on_demand_book);

        let mut buffer = default_vec_buffer();
        let arena = deserialization_arena!(&mut buffer);
        match deserialize_advertisement::<
            ChainedCredentialBook<
                &PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>,
                &PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>,
                LazyDiscoveryCredentialBook,
            >,
            VecBuffer,
            C,
        >(arena, data, &chains)?
        {
            DeserializedAdvertisement::V0(v0) => match v0 {
                V0AdvertisementContents::Plaintext(adv_contents) => {
                    decode_v0_plaintext(adv_contents)
                }
                V0AdvertisementContents::Decrypted(with_matched_credential) => {
                    self.decode_v0_decrypted(with_matched_credential)
                }
                V0AdvertisementContents::NoMatchingCredentials => {
                    log::info!("No matching credentials, failed to decode V0 advertisement.");
                    Ok(DecodedResult::NotDecoded)
                }
            },
            DeserializedAdvertisement::V1(adv_contents) => self.decode_v1(adv_contents),
        }
    }

    fn decode_v0_decrypted(
        &self,
        with_matched_credential: WithMatchedCredential<
            MatchedCredentialWithId<Vec<u8>>,
            DecryptedAdvContents,
        >,
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        use credential_proto::proto::metadata::DeviceIdentityMetaData;
        use identity_common::definitions::VisibilityLevel;
        use identity_manager::errors::CredentialsError;
        use protobuf::Message as _;
        if let DecodedDataElements::UP { data_elements, .. } =
            decode_v0_data_elements(with_matched_credential.contents().data_elements())?
        {
            let id = with_matched_credential.matched_credential().id();
            let visibility_scope = self
                .identity_manager
                .get_visibility_scope_from_id(id)
                .ok_or(CredentialsError::CredentialNotFound)?;
            if visibility_scope.level() == VisibilityLevel::Contact {
                self.identity_manager.notify_contact_credential_used(id);
            }
            let decrypted_metadata =
                with_matched_credential.decrypt_metadata::<C>().map_err(|err| match err {
                    MatchedMetadataDecryptionError::RetrievalFailed(source_err) => {
                        DecodingWithVisibilityError::MetadataRetrievalFailed(format!(
                            "{source_err:?}"
                        ))
                    }
                    MatchedMetadataDecryptionError::DecryptionFailed => {
                        DecodingWithVisibilityError::MetadataDecryptionFailed
                    }
                })?;

            let decoded_metadata = DeviceIdentityMetaData::parse_from_bytes(&decrypted_metadata)?;
            let device_name = decoded_metadata.device_name;
            let device_type: DeviceType =
                decoded_metadata.device_type.enum_value_or_default().into();
            let device_unique_id =
                DeviceUniqueId::Stable(decoded_metadata.device_id.as_slice().try_into()?);
            let decoded_info = DecodedDeviceInfo {
                device_name,
                is_name_truncated: false,
                device_image_url: None,
                device_type,
                auth_token: None, // TODO: implement auth token.
                visibility_scope,
                data_elements,
            };
            Ok(DecodedResult::V0 { device_unique_id, decoded_info })
        } else {
            Err(DecodingWithVisibilityError::InvalidPublicV1Frame)
        }
    }

    fn decode_v1(
        &self,
        adv_contents: V1AdvertisementContents<MatchedCredentialWithId<Vec<u8>>>,
    ) -> Result<DecodedResult, DecodingWithVisibilityError> {
        let mut decoded_info_list = Vec::new();
        let mut device_unique_id: Option<DeviceUniqueId> = None;
        for section in adv_contents.sections() {
            match section {
                V1DeserializedSection::Plaintext(plaintext_section) => {
                    match decode_v1_data_elements(plaintext_section.iter_data_elements())? {
                        DecodedDataElements::UP {
                            device_name,
                            is_name_truncated,
                            device_type,
                            broadcast_session_id,
                            data_elements,
                        } => {
                            let broadcast_session_id = broadcast_session_id
                                .ok_or(DecodingWithVisibilityError::MissingBroadcastSessionId)?;
                            let decoded_info = DecodedDeviceInfo {
                                data_elements,
                                device_name,
                                is_name_truncated,
                                device_image_url: None,
                                device_type,
                                auth_token: None,
                                visibility_scope: VisibilityScope::Public,
                            };
                            decoded_info_list.push(decoded_info);
                            if device_unique_id.is_none() {
                                device_unique_id =
                                    Some(DeviceUniqueId::Public(broadcast_session_id));
                            }
                        }
                        DecodedDataElements::Control { data_elements } => {
                            if adv_contents.sections().size_hint().0 > 1 {
                                return Err(DecodingWithVisibilityError::InvalidPublicV1Frame);
                            }
                            return Ok(DecodedResult::Control { data_elements });
                        }
                    }
                }
                V1DeserializedSection::Decrypted(decrypted_section) => {
                    let (unique_id, decoded_info) = self.decode_v1_decrypted(decrypted_section)?;
                    decoded_info_list.push(decoded_info);
                    // Do not overwrite ID if it's already a stable device ID.
                    if device_unique_id.is_none()
                        || !matches!(device_unique_id, Some(DeviceUniqueId::Stable(_)))
                    {
                        device_unique_id = Some(unique_id);
                    }
                }
            };
        }
        let device_unique_id =
            device_unique_id.ok_or(DecodingWithVisibilityError::MissingBroadcastSessionId)?;
        Ok(DecodedResult::V1 {
            device_unique_id,
            decoded_info_list,
            invalid_sections_count: adv_contents.invalid_sections_count(),
        })
    }

    fn decode_v1_decrypted(
        &self,
        decrypted_section: &WithMatchedCredential<
            MatchedCredentialWithId<Vec<u8>>,
            DecryptedSection,
        >,
    ) -> Result<(DeviceUniqueId, DecodedDeviceInfo), DecodingWithVisibilityError> {
        use credential_proto::proto::metadata::DeviceIdentityMetaData;
        use identity_common::definitions::VisibilityLevel;
        use identity_manager::errors::CredentialsError;
        use protobuf::Message as _;
        use up_adv::extended::deserialize::Section as _;

        if let DecodedDataElements::UP { data_elements, broadcast_session_id, .. } =
            decode_v1_data_elements(decrypted_section.contents().iter_data_elements())?
        {
            let id = decrypted_section.matched_credential().id();
            let visibility_scope = self
                .identity_manager
                .get_visibility_scope_from_id(id)
                .ok_or(CredentialsError::CredentialNotFound)?;
            if visibility_scope.level() == VisibilityLevel::Contact {
                self.identity_manager.notify_contact_credential_used(id);
            }
            let decrypted_metadata =
                decrypted_section.decrypt_metadata::<C>().map_err(|err| match err {
                    MatchedMetadataDecryptionError::RetrievalFailed(source_err) => {
                        DecodingWithVisibilityError::MetadataRetrievalFailed(format!(
                            "{source_err:?}"
                        ))
                    }
                    MatchedMetadataDecryptionError::DecryptionFailed => {
                        DecodingWithVisibilityError::MetadataDecryptionFailed
                    }
                })?;
            let decoded_metadata = DeviceIdentityMetaData::parse_from_bytes(&decrypted_metadata)?;
            let device_name = decoded_metadata.device_name;
            let device_type: DeviceType =
                decoded_metadata.device_type.enum_value_or_default().into();
            // NOTE: Only SELF scope credentials contain a stable device ID, else we
            // expect to see a broadcast session ID.
            let unique_id = if visibility_scope.level() == VisibilityLevel::SelfScope {
                DeviceUniqueId::Stable(decoded_metadata.device_id.as_slice().try_into()?)
            } else if let Some(session_id) = broadcast_session_id {
                DeviceUniqueId::Public(session_id)
            } else {
                return Err(DecodingWithVisibilityError::MissingBroadcastSessionId);
            };

            Ok((
                unique_id,
                DecodedDeviceInfo {
                    device_name,
                    is_name_truncated: false,
                    device_image_url: None,
                    device_type,
                    auth_token: None, // TODO: implement auth token.
                    visibility_scope,
                    data_elements,
                },
            ))
        } else {
            Err(DecodingWithVisibilityError::InvalidPublicV1Frame)
        }
    }
}

fn decode_v0_plaintext(
    adv_contents: UnencryptedAdvContents,
) -> Result<DecodedResult, DecodingWithVisibilityError> {
    match decode_v0_data_elements(adv_contents.data_elements())? {
        DecodedDataElements::UP {
            device_name,
            is_name_truncated,
            device_type,
            broadcast_session_id,
            data_elements,
        } => {
            let device_unique_id = if let Some(id) = broadcast_session_id {
                DeviceUniqueId::Public(id)
            } else {
                return Err(DecodingWithVisibilityError::MissingBroadcastSessionId);
            };
            let decoded_info = DecodedDeviceInfo {
                device_name,
                is_name_truncated,
                device_image_url: None,
                device_type,
                auth_token: None,
                visibility_scope: VisibilityScope::Public,
                data_elements,
            };
            Result::Ok(DecodedResult::V0 { device_unique_id, decoded_info })
        }
        DecodedDataElements::Control { data_elements } => {
            Result::Ok(DecodedResult::Control { data_elements })
        }
    }
}

fn decode_v0_data_elements<F: PacketFlavor>(
    de_iterator: DeIterator<F>,
) -> Result<DecodedDataElements, DecodingWithVisibilityError> {
    let mut data_elements = Vec::new();
    let mut device_type: DeviceType = DeviceType::Unknown;
    let mut is_name_truncated: bool = false;
    let mut device_name: String = "".to_string();
    let mut broadcast_session_id: Option<BroadcastSessionId> = None;
    let mut is_control_frame = false;
    let deserialized_data_elements = de_iterator
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| DecodingWithVisibilityError::DataElementDeserializeError { error: e })?;
    for data_element in deserialized_data_elements {
        match data_element {
            DeserializedDataElement::Actions(actions_de) => {
                let mut service_ids = vec![];
                for action_type in actions_de.action.actions() {
                    service_ids.push(ServiceId(action_type.0 as u32));
                }
                data_elements.push(CoreDataElement::ServiceIds(service_ids));
            }
            DeserializedDataElement::TxPower(tx_power_de) => {
                data_elements.push(CoreDataElement::BleTxPower(tx_power_de.tx_power.as_i8()));
            }
            DeserializedDataElement::Psm(psm_de) => {
                data_elements.push(CoreDataElement::BleL2capCocPsm(psm_de.value));
            }
            DeserializedDataElement::DeviceInfo(device_info_de) => {
                device_type = device_info_de.device_info.device_type();
                is_name_truncated = device_info_de.device_info.name_truncated();
                device_name =
                    String::from_utf8(Vec::from(device_info_de.device_info.device_name()))
                        .map_err(|_| DecodingWithVisibilityError::DeviceNameDecodingFailure)?;
            }
            DeserializedDataElement::DedupHint(dedup_hint_de) => {
                broadcast_session_id =
                    Some(BroadcastSessionId::new(dedup_hint_de.dedup_hint.as_bytes()));
            }
            DeserializedDataElement::Requirements(requirements_de) => {
                is_control_frame = true;
                data_elements.push(CoreDataElement::Requirements(parse_capability_bitset(
                    &requirements_de.into(),
                )));
            }
            DeserializedDataElement::DiscoveryType(discovery_type_de) => {
                data_elements
                    .push(CoreDataElement::DiscoveryType(discovery_type_de.discovery_type));
            }
        }
    }
    if is_control_frame {
        Ok(DecodedDataElements::Control { data_elements })
    } else {
        Ok(DecodedDataElements::UP {
            device_name,
            is_name_truncated,
            device_type,
            broadcast_session_id,
            data_elements,
        })
    }
}

fn decode_v1_data_elements(
    de_iterator: DataElementParsingIterator,
) -> Result<DecodedDataElements, DecodingWithVisibilityError> {
    use up_adv::extended::data_elements::DeviceNameDataElement;
    use up_adv::extended::data_elements::DeviceTypeDataElement;
    use up_adv::extended::data_elements::TxPowerDataElement;

    let mut data_elements = vec![];
    let mut device_type: DeviceType = DeviceType::Unknown;
    let mut device_name: String = "".to_string();
    let mut is_name_truncated: bool = false;
    let mut broadcast_session_id: Option<BroadcastSessionId> = None;
    let mut is_control_frame = false;
    for de in de_iterator.collect::<Result<Vec<_>, _>>()? {
        match de.de_type() {
            TxPowerDataElement::DE_TYPE => {
                let tx_power_de: TxPower =
                    TxPowerDataElement::try_deserialize(None, de.contents())?.into();
                let tx_power = CoreDataElement::BleTxPower(tx_power_de.as_i8());
                data_elements.push(tx_power);
            }
            up_adv::extended::data_elements::ActionsDataElement::DE_TYPE => {
                let mut service_ids = vec![];
                let deserialized_actions =
                    DeserializedActionsDE::try_deserialize(None, de.contents())?;
                for action in deserialized_actions.collect_action_ids() {
                    service_ids.push(ServiceId(action?.as_u16().into()));
                }
                data_elements.push(CoreDataElement::ServiceIds(service_ids));
            }
            DeviceNameDataElement::DE_TYPE => {
                let device_name_de = DeviceNameDataElement::try_deserialize(None, de.contents())?;
                device_name = device_name_de.name().to_string();
                is_name_truncated = false; // Name is never truncated in extended format.
            }
            DeviceTypeDataElement::DE_TYPE => {
                let device_type_de = DeviceTypeDataElement::try_deserialize(None, de.contents())?;
                device_type = DeviceType::try_from(device_type_de).unwrap_or(DeviceType::Unknown);
            }
            ConnectivityInfoDataElement::DE_TYPE => {
                let conn_info = ConnectivityInfoDataElement::try_deserialize(None, de.contents())?;
                if conn_info.ble_info != BleConnectivityInfo::default() {
                    let ble_info = conn_info.ble_info;
                    data_elements.push(CoreDataElement::BleConnectionInformation {
                        mac_address: ble_info.mac_address.map_or(vec![], |a| a.to_vec()),
                        gatt_characteristic: ble_info
                            .gatt_service_identifier
                            .map_or(vec![], |a| a.to_vec()),
                        psm: ble_info.psm.map_or(0, u16::from_be_bytes),
                        device_token: ble_info.device_token.map_or(0, u16::from_be_bytes),
                    });
                }
                if conn_info.wifi_lan_info != WifiLanConnectivityInfo::default() {
                    let wifi_info = conn_info.wifi_lan_info;
                    if let Some(ip) = wifi_info.ip {
                        match ip {
                            IpAddr::V4(ipv4) => {
                                data_elements.push(
                                    CoreDataElement::WifiLanIpv4ConnectionInformation {
                                        ip_address: ipv4.octets().to_vec(),
                                        port: wifi_info.port.unwrap_or(0),
                                        bssid: wifi_info.bssid.map(|b| b.to_vec()),
                                    },
                                );
                            }
                            IpAddr::V6(ipv6) => {
                                data_elements.push(
                                    CoreDataElement::WifiLanIpv6ConnectionInformation {
                                        ip_address: ipv6.octets().to_vec(),
                                        port: wifi_info.port.unwrap_or(0),
                                        bssid: wifi_info.bssid.map(|b| b.to_vec()),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            ContextSyncSeqNumDataElement::DE_TYPE => {
                let seq_num: ContextSyncSeqNum =
                    ContextSyncSeqNumDataElement::try_deserialize(None, de.contents())?.into();
                data_elements.push(CoreDataElement::ContextSequenceNumber(seq_num.as_u8()));
            }
            CastIdDataElement::DE_TYPE => {
                let cast_id = CastIdDataElement::try_deserialize(None, de.contents())?;
                data_elements.push(CoreDataElement::GoogleCastId(*cast_id.get_id_as_bytes()));
            }
            MediaDeduplicationIdDataElement::DE_TYPE => {
                let media_dedupe_id =
                    MediaDeduplicationIdDataElement::try_deserialize(None, de.contents())?;
                data_elements.push(CoreDataElement::MediaDeduplicationId(
                    *media_dedupe_id.get_id_as_bytes(),
                ));
            }
            BroadcastSessionIdDataElement::DE_TYPE => {
                let session_id =
                    BroadcastSessionIdDataElement::try_deserialize(None, de.contents())?
                        .broadcast_session_id;
                broadcast_session_id = Some(BroadcastSessionId::new(session_id));
            }
            CapabilitiesDataElement::DE_TYPE => {
                let caps_de = CapabilitiesDataElement::try_deserialize(None, de.contents())?;
                data_elements
                    .push(CoreDataElement::Capabilities(parse_capability_bitset(&caps_de.into())));
            }
            RequirementsDataElement::DE_TYPE => {
                is_control_frame = true;
                let reqs_de = RequirementsDataElement::try_deserialize(None, de.contents())?;
                data_elements
                    .push(CoreDataElement::Requirements(parse_capability_bitset(&reqs_de.into())));
            }
            DiscoveryTypeDataElement::DE_TYPE => {
                let discovery_type_de =
                    DiscoveryTypeDataElement::try_deserialize(None, de.contents())?;
                data_elements
                    .push(CoreDataElement::DiscoveryType(discovery_type_de.discovery_type));
            }
            _ => { /* TODO: pass other DEs. */ }
        }
    }
    if is_control_frame {
        Ok(DecodedDataElements::Control { data_elements })
    } else {
        Ok(DecodedDataElements::UP {
            device_name,
            is_name_truncated,
            device_type,
            broadcast_session_id,
            data_elements,
        })
    }
}

// Parses a `CapabilityBitsetDataElement` into a vec of `DeviceCapability`.
fn parse_capability_bitset(
    bitset: &up_adv::shared_data::CapabilityBitset,
) -> Vec<DeviceCapability> {
    [
        DeviceCapability::InternetConnectivity,
        DeviceCapability::CastReceiver,
        DeviceCapability::Camera,
        DeviceCapability::Speaker,
    ]
    .into_iter()
    .filter(|&cap| bitset.has(cap))
    .collect()
}
