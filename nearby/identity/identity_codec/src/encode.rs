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
    errors::{
        EncodingWithVisibilityError,
        InvalidV1EncodingRequest::{
            InvalidConnectivityInfoDataElement, InvalidPublicNonPublicMerge,
        },
    },
    types::AdvertisementServiceData,
};
use common::data_elements::{CoreDataElement, DeviceCapability};
use crypto_provider::{tinyvec, CryptoProvider, CryptoRng, FromCryptoRng};
use identity_common::definitions::VisibilityScope;
use std::{collections::HashMap, net::IpAddr};
use up_adv::{
    buffer::VecBuffer,
    default_vec_buffer,
    extended::{
        data_elements::{
            BleConnectivityInfo, DeviceNameDataElement, DeviceTypeDataElement,
            WifiLanConnectivityInfo,
        },
        serialize::{AdvBuilder as ExtendedAdvBuilder, SectionEncoder, UnencryptedSectionEncoder},
    },
    legacy::{
        data_elements::actions::ActionBits,
        serialize::{AdvBuilder as LegacyAdvBuilder, UnencryptedEncoder},
    },
    shared_data::{DedupHint, TxPower, MAX_DEVICE_NAME_LEN},
};

#[cfg(feature = "encrypted_identity")]
impl<C: CryptoProvider + Send + Sync + 'static> CodecImpl<C>
where
    C::CryptoRng: CryptoRng,
{
    pub(super) fn encode_v0_encrypted(
        &self,
        visibility: &identity_common::definitions::VisibilityLevel,
        data_elements: &[CoreDataElement],
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
        use up_adv::legacy::{data_elements::psm::PsmDataElement, serialize::LdtEncoder};

        let (credential, salt) =
            self.identity_manager.get_v0_broadcast_credential_with_salt(visibility)?;
        let encoder = LdtEncoder::<C>::new(salt, &credential);
        let mut builder = LegacyAdvBuilder::new(encoder);

        for element in data_elements {
            match element {
                CoreDataElement::BleTxPower(tx_power) => {
                    builder.add_data_element(
                        up_adv::legacy::data_elements::tx_power::TxPowerDataElement::from(
                            TxPower::try_from(*tx_power)?,
                        ),
                    )?;
                }
                CoreDataElement::BleL2capCocPsm(psm) => {
                    builder.add_data_element(PsmDataElement { value: *psm })?;
                }
                CoreDataElement::ServiceIds(service_ids) => {
                    let mut action_bits = ActionBits::<up_adv::legacy::Ciphertext>::default();
                    for id in service_ids {
                        use up_adv::legacy::data_elements::actions::ActionType;

                        action_bits.set_action(ActionType(id.0 as u8));
                    }
                    builder.add_data_element(
                        up_adv::legacy::data_elements::actions::ActionsDataElement::from(
                            action_bits,
                        ),
                    )?;
                }
                _ => {
                    // TODO: implement remaining elements
                    // do nothing
                }
            }
        }
        let serialized_adv = builder
            .into_advertisement()
            .map_err(|e| EncodingWithVisibilityError::LdtEncodeError { error: e })?;
        Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() })
    }

    pub(super) fn encode_v1(
        &self,
        data_elements: &HashMap<VisibilityScope, Vec<CoreDataElement>>,
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
        let buffer = default_vec_buffer();
        let mut adv_builder = ExtendedAdvBuilder::new(buffer);
        if let Some(elements) = data_elements.get(&VisibilityScope::Public) {
            if data_elements.len() > 1 {
                return Err(EncodingWithVisibilityError::InvalidV1EncodingRequest {
                    source: InvalidPublicNonPublicMerge,
                });
            }
            self.encode_v1_plaintext(&mut adv_builder, elements)?;
            let serialized_adv = adv_builder.into_advertisement();
            return Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() });
        }
        for (scope, elements) in data_elements {
            self.encode_v1_encrypted(&mut adv_builder, scope, elements)?;
        }
        let serialized_adv = adv_builder.into_advertisement();
        Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() })
    }

    fn encode_v1_encrypted(
        &self,
        adv_builder: &mut ExtendedAdvBuilder<VecBuffer>,
        visibility_scope: &VisibilityScope,
        data_elements: &[CoreDataElement],
    ) -> Result<(), EncodingWithVisibilityError> {
        use up_adv::extended::{salt::ShortV1Salt, serialize::MicEncryptedSectionEncoder};
        let credential = self.identity_manager.get_v1_broadcast_credential(visibility_scope)?;

        // Use ShortV1Salt for now as AdvBuilder doesn't support Root Salt yet.
        let mut crypto_rng = <C>::CryptoRng::new();
        let salt = ShortV1Salt::new_random(&mut crypto_rng);

        let mut section_builder = adv_builder
            .section_builder(MicEncryptedSectionEncoder::new_with_salt::<C>(salt, &credential))?;
        add_data_elements_to_section_builder::<_>(data_elements, &mut section_builder)?;
        if visibility_scope.level() != identity_common::definitions::VisibilityLevel::SelfScope {
            // Add broadcast session ID for non-SELF scope encrypted advertisements.
            let broadcast_session_id =
                *self.identity_manager.local_broadcast_session_id().as_bytes();
            let broadcast_session_id_de =
                up_adv::extended::data_elements::BroadcastSessionIdDataElement::from(
                    broadcast_session_id,
                );
            section_builder.add_de(&broadcast_session_id_de)?;
        }
        section_builder.add_to_advertisement::<C>();
        Ok(())
    }
}

#[cfg(not(feature = "encrypted_identity"))]
impl<C: CryptoProvider + Send + Sync + 'static> CodecImpl<C>
where
    C::CryptoRng: CryptoRng,
{
    pub(super) fn encode_v1_public_only(
        &self,
        data_elements: &HashMap<VisibilityScope, Vec<CoreDataElement>>,
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
        let buffer = default_vec_buffer();
        let mut adv_builder = ExtendedAdvBuilder::new(buffer);
        if let Some(elements) = data_elements.get(&VisibilityScope::Public) {
            if data_elements.len() > 1 {
                return Err(EncodingWithVisibilityError::InvalidV1EncodingRequest {
                    source: InvalidPublicNonPublicMerge,
                });
            }
            self.encode_v1_plaintext(&mut adv_builder, elements)?;
            let serialized_adv = adv_builder.into_advertisement();
            return Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() });
        }
        Err(EncodingWithVisibilityError::UnsupportedApiRequest)
    }
}

/// CodecImpl for plaintext encoding.
impl<C: CryptoProvider + Send + Sync + 'static> CodecImpl<C>
where
    C::CryptoRng: CryptoRng,
{
    pub(super) fn encode_v0_plaintext(
        &self,
        data_elements: &[CoreDataElement],
    ) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
        use up_adv::legacy::data_elements::psm::PsmDataElement;
        use up_adv::legacy::data_elements::tx_power::TxPowerDataElement;

        let mut builder = LegacyAdvBuilder::new(UnencryptedEncoder);
        for element in data_elements {
            match element {
                CoreDataElement::BleTxPower(tx_power) => {
                    builder.add_data_element(TxPowerDataElement::from(TxPower::try_from(
                        *tx_power,
                    )?))?;
                }
                CoreDataElement::BleL2capCocPsm(psm) => {
                    builder.add_data_element(PsmDataElement { value: *psm })?;
                }
                CoreDataElement::ServiceIds(service_ids) => {
                    let mut action_bits = ActionBits::<up_adv::legacy::Plaintext>::default();
                    for id in service_ids {
                        use up_adv::legacy::data_elements::actions::ActionType;

                        action_bits.set_action(ActionType(id.0 as u8));
                    }
                    builder.add_data_element(
                        up_adv::legacy::data_elements::actions::ActionsDataElement::from(
                            action_bits,
                        ),
                    )?;
                }
                _ => {
                    // TODO: implement remaining elements
                    // do nothing
                }
            }
        }

        // Add device info DE.
        let device_type = self.identity_manager.local_device_type();
        let device_name = self.identity_manager.local_device_name();
        let (truncated_device_name, is_name_truncated) =
            truncate_device_name(device_name.as_bytes());
        let device_info = up_adv::shared_data::DeviceInfo::try_from((
            device_type,
            is_name_truncated,
            truncated_device_name.as_slice(),
        ))?;
        builder.add_data_element(
            up_adv::legacy::data_elements::device_info::DeviceInfoDataElement::from(device_info),
        )?;

        // Add dedup hint DE.
        let broadcast_session_id = *self.identity_manager.local_broadcast_session_id().as_bytes();
        builder.add_data_element(
            up_adv::legacy::data_elements::dedup_hint::DeduplicationHintDataElement::from(
                DedupHint::from(broadcast_session_id),
            ),
        )?;

        let serialized_adv = builder
            .into_advertisement()
            .map_err(|e| EncodingWithVisibilityError::UnencryptedEncodeError { error: e })?;
        Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() })
    }

    fn encode_v1_plaintext(
        &self,
        builder: &mut ExtendedAdvBuilder<VecBuffer>,
        data_elements: &[CoreDataElement],
    ) -> Result<(), EncodingWithVisibilityError> {
        let mut section_builder = builder.section_builder(UnencryptedSectionEncoder)?;
        self.add_public_only_data_elements::<_>(&mut section_builder)?;
        add_data_elements_to_section_builder::<_>(data_elements, &mut section_builder)?;
        section_builder.add_to_advertisement::<C>();
        Ok(())
    }

    fn add_public_only_data_elements<S: SectionEncoder>(
        &self,
        section_builder: &mut up_adv::extended::serialize::SectionBuilder<
            &mut ExtendedAdvBuilder<VecBuffer>,
            S,
            VecBuffer,
        >,
    ) -> Result<(), EncodingWithVisibilityError> {
        let device_name = self.identity_manager.local_device_name();
        // Add device name.
        let device_name_de = DeviceNameDataElement::new(&device_name);
        section_builder.add_de(&device_name_de)?;

        // Add device type.
        let device_type = self.identity_manager.local_device_type();
        let device_type_de = DeviceTypeDataElement::from(device_type);
        section_builder.add_de(&device_type_de)?;

        // Add broadcast session ID.
        let broadcast_session_id = *self.identity_manager.local_broadcast_session_id().as_bytes();
        let broadcast_session_id_de =
            up_adv::extended::data_elements::BroadcastSessionIdDataElement::from(
                broadcast_session_id,
            );
        section_builder.add_de(&broadcast_session_id_de)?;
        Ok(())
    }
}

fn add_data_elements_to_section_builder<S: SectionEncoder>(
    data_elements: &[CoreDataElement],
    section_builder: &mut up_adv::extended::serialize::SectionBuilder<
        &mut ExtendedAdvBuilder<VecBuffer>,
        S,
        VecBuffer,
    >,
) -> Result<(), EncodingWithVisibilityError> {
    use up_adv::extended::data_elements::TxPowerDataElement;

    let mut ble_info: Option<BleConnectivityInfo> = None;
    let mut wifi_lan_info: Option<WifiLanConnectivityInfo> = None;
    for element in data_elements {
        match element {
            CoreDataElement::BleTxPower(tx_power) => {
                let tx_power_de = TxPowerDataElement::from(TxPower::try_from(*tx_power)?);
                section_builder.add_de(&tx_power_de)?;
            }
            CoreDataElement::ServiceIds(service_ids) => {
                let mut actions = tinyvec::ArrayVec::<
                    [up_adv::extended::data_elements::actions::ActionId; 64],
                >::new();
                for id in service_ids {
                    if let Ok(action_id) = (id.0 as u16).try_into() {
                        actions.push(action_id);
                    }
                }
                let actions_de =
                    up_adv::extended::data_elements::ActionsDataElement::try_from_actions(actions)?;
                section_builder.add_de(&actions_de)?;
            }
            CoreDataElement::BleConnectionInformation {
                mac_address,
                gatt_characteristic,
                psm,
                device_token,
            } => {
                ble_info = Some(BleConnectivityInfo {
                    mac_address: mac_address.as_slice().try_into().ok(),
                    gatt_service_identifier: if gatt_characteristic.is_empty() {
                        None
                    } else {
                        let mut arr = tinyvec::ArrayVec::new();
                        arr.extend_from_slice(gatt_characteristic.as_slice());
                        Some(arr)
                    },
                    psm: Some(psm.to_be_bytes()),
                    device_token: Some(device_token.to_be_bytes()),
                });
            }
            CoreDataElement::WifiLanIpv4ConnectionInformation { ip_address, port, bssid } => {
                wifi_lan_info = Some(create_wifi_lan_info_de(ip_address, port, bssid, 4)?);
            }
            CoreDataElement::WifiLanIpv6ConnectionInformation { ip_address, port, bssid } => {
                // TODO: check if Wifi Ipv4 was set, and return invalid request if so. Request should only contain one wifi ip addr.
                wifi_lan_info = Some(create_wifi_lan_info_de(ip_address, port, bssid, 16)?);
            }
            CoreDataElement::ContextSequenceNumber(num) => {
                let context_sequence_number_de =
                    up_adv::extended::data_elements::ContextSyncSeqNumDataElement::from(
                        up_adv::shared_data::ContextSyncSeqNum::try_from(*num)?,
                    );
                section_builder.add_de(&context_sequence_number_de)?;
            }
            CoreDataElement::GoogleCastId(id) => {
                let cast_id_de = up_adv::extended::data_elements::CastIdDataElement::from(id);
                section_builder.add_de(&cast_id_de)?;
            }
            CoreDataElement::MediaDeduplicationId(id) => {
                let media_dedupe_id_de =
                    up_adv::extended::data_elements::MediaDeduplicationIdDataElement::from(id);
                section_builder.add_de(&media_dedupe_id_de)?;
            }
            CoreDataElement::Capabilities(capabilities) => {
                let bitset = create_capability_bitset(capabilities);
                section_builder.add_de(
                    &up_adv::extended::data_elements::CapabilitiesDataElement::from(bitset),
                )?;
            }
            CoreDataElement::Requirements(requirements) => {
                let bitset = create_capability_bitset(requirements);
                section_builder.add_de(
                    &up_adv::extended::data_elements::RequirementsDataElement::from(bitset),
                )?;
            }
            _ => {
                // TODO: implement remaining elements
            }
        }
    }
    if ble_info.is_some() || wifi_lan_info.is_some() {
        let conn_info_de = up_adv::extended::data_elements::ConnectivityInfoDataElement {
            ble_info: ble_info.unwrap_or_default(),
            wifi_lan_info: wifi_lan_info.unwrap_or_default(),
        };
        if conn_info_de.is_some() {
            section_builder.add_de(&conn_info_de)?;
        }
    }
    Ok(())
}

fn create_wifi_lan_info_de(
    ip_address: &Vec<u8>,
    port: &u16,
    bssid: &Option<Vec<u8>>,
    ip_address_size: usize,
) -> Result<WifiLanConnectivityInfo, EncodingWithVisibilityError> {
    if ip_address.len() != ip_address_size {
        return Err(EncodingWithVisibilityError::InvalidV1EncodingRequest {
            source: InvalidConnectivityInfoDataElement(
                "Invalid Wifi LAN IP address length!".to_string(),
            ),
        });
    }
    let ip = match ip_address_size {
        4 => {
            #[allow(clippy::unwrap_used)]
            let ipv4_array: [u8; 4] = ip_address.as_slice().try_into().unwrap();
            IpAddr::from(ipv4_array)
        }
        16 => {
            #[allow(clippy::unwrap_used)]
            let ipv6_array: [u8; 16] = ip_address.as_slice().try_into().unwrap();
            IpAddr::from(ipv6_array)
        }
        _ => {
            return Err(EncodingWithVisibilityError::InvalidV1EncodingRequest {
                source: InvalidConnectivityInfoDataElement(
                    "Unsupported IP address size!".to_string(),
                ),
            });
        }
    };
    let bssid = if let Some(bssid_vec) = bssid {
        let bssid_array: [u8; 6] = bssid_vec.as_slice().try_into().map_err(|_| {
            EncodingWithVisibilityError::InvalidV1EncodingRequest {
                source: InvalidConnectivityInfoDataElement(
                    "Invalid Wifi BSSID length!".to_string(),
                ),
            }
        })?;
        Some(bssid_array)
    } else {
        None
    };
    Ok(WifiLanConnectivityInfo { ip: Some(ip), port: Some(*port), bssid })
}

// Creates a `CapabilityBitsetDataElement` from a slice of `DeviceCapability`.
fn create_capability_bitset(
    capabilities: &[DeviceCapability],
) -> up_adv::shared_data::CapabilityBitset {
    let mut bitset = up_adv::shared_data::CapabilityBitset::default();
    for cap in capabilities {
        bitset.add(*cap);
    }
    bitset
}

/// Truncates the device name to 9 bytes if it's longer, returning the truncated name
/// and a boolean indicating if truncation occurred.
fn truncate_device_name(device_name: &[u8]) -> (Vec<u8>, bool) {
    if device_name.len() > MAX_DEVICE_NAME_LEN {
        #[allow(clippy::indexing_slicing)]
        (device_name[..MAX_DEVICE_NAME_LEN].to_vec(), true)
    } else {
        (device_name.to_vec(), false)
    }
}

pub(super) fn encode_dct_ios(
    _data_elements: &[CoreDataElement],
) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
    todo!()
}

pub(super) fn encode_control<C: CryptoProvider>(
    short_format: bool,
    data_elements: &[CoreDataElement],
) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
    if short_format {
        encode_control_short_format(data_elements)
    } else {
        encode_control_long_format::<C>(data_elements)
    }
}

fn encode_control_short_format(
    data_elements: &[CoreDataElement],
) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
    let mut builder = LegacyAdvBuilder::new(UnencryptedEncoder);
    for element in data_elements {
        match element {
            CoreDataElement::ServiceIds(service_ids) => {
                let mut action_bits = ActionBits::<up_adv::legacy::Plaintext>::default();
                for id in service_ids {
                    use up_adv::legacy::data_elements::actions::ActionType;

                    action_bits.set_action(ActionType(id.0 as u8));
                }
                builder.add_data_element(
                    up_adv::legacy::data_elements::actions::ActionsDataElement::from(action_bits),
                )?;
            }
            CoreDataElement::DiscoveryType(discovery_type) => {
                builder.add_data_element(
                    up_adv::legacy::data_elements::discovery_type::DiscoveryTypeDataElement::from(
                        *discovery_type,
                    ),
                )?;
            }
            CoreDataElement::Requirements(requirements) => {
                let bitset = create_capability_bitset(requirements);
                builder.add_data_element(
                    up_adv::legacy::data_elements::requirements::RequirementsDataElement::from(
                        bitset,
                    ),
                )?;
            }
            _ => {
                // do nothing
            }
        }
    }

    let serialized_adv = builder
        .into_advertisement()
        .map_err(|e| EncodingWithVisibilityError::UnencryptedEncodeError { error: e })?;
    Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() })
}

fn encode_control_long_format<C: CryptoProvider>(
    data_elements: &[CoreDataElement],
) -> Result<AdvertisementServiceData, EncodingWithVisibilityError> {
    let buffer = default_vec_buffer();
    let mut adv_builder = ExtendedAdvBuilder::new(buffer);
    let mut section_builder = adv_builder.section_builder(UnencryptedSectionEncoder)?;
    add_data_elements_to_section_builder::<_>(data_elements, &mut section_builder)?;
    section_builder.add_to_advertisement::<C>();
    let serialized_adv = adv_builder.into_advertisement();
    Ok(AdvertisementServiceData { data: serialized_adv.as_slice().to_vec() })
}
