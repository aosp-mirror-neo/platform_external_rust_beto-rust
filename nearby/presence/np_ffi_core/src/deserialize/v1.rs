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
//! Core NP Rust FFI structures and methods for v1 advertisement deserialization.

use super::DeserializeAdvertisementError;
use crate::{
    common::*,
    credentials::{CredentialBook, MatchedCredential},
    deserialize::{allocate_decrypted_metadata_handle, DecryptMetadataResult},
    utils::*,
    v1::*,
    CryptoProviderImpl,
};
use array_view::ArrayView;
use crypto_provider::CryptoProvider;
use handle_map::{declare_handle_map, HandleLike};
use np_adv::{
    credential::matched::WithMatchedCredential,
    extended::{
        deserialize::{
            data_element::DataElementParseError, V1AdvertisementContents, V1DeserializedSection,
        },
        salt::MultiSalt,
    },
};

/// Representation of a deserialized V1 advertisement
#[repr(C)]
pub struct DeserializedV1Advertisement {
    /// The number of legible sections
    pub num_legible_sections: u8,
    /// The number of sections that were unable to be decrypted
    pub num_undecryptable_sections: u8,
    /// A handle to the set of legible (plain or decrypted) sections
    pub legible_sections: LegibleV1Sections,
}

impl DeserializedV1Advertisement {
    /// Gets the number of legible sections in this deserialized V1 advertisement.
    pub fn num_legible_sections(&self) -> u8 {
        self.num_legible_sections
    }

    /// Gets the number of undecryptable sections in this deserialized V1 advertisement.
    pub fn num_undecryptable_sections(&self) -> u8 {
        self.num_undecryptable_sections
    }

    /// Gets the legible section with the given index (which is bounded in
    /// `0..self.num_legible_sections()`). This uses the internal handle but does not take
    /// ownership of it.
    pub fn get_section(&self, legible_section_index: u8) -> GetV1SectionResult {
        match self.legible_sections.get() {
            Ok(sections_read_guard) => {
                sections_read_guard.get_section(self.legible_sections, legible_section_index)
            }
            Err(_) => GetV1SectionResult::Error,
        }
    }

    /// Attempts to deallocate memory utilized internally by this V1 advertisement (which contains
    /// a handle to actual advertisement contents behind-the-scenes). This function takes ownership
    /// of the internal handle.
    pub fn deallocate(self) -> DeallocateResult {
        self.legible_sections.deallocate().map(|_| ()).into()
    }

    pub(crate) fn allocate_with_contents(
        contents: V1AdvertisementContents<
            np_adv::credential::matched::ReferencedMatchedCredential<MatchedCredential>,
        >,
    ) -> Result<Self, DeserializeAdvertisementError> {
        // 16-section limit enforced by np_adv
        let num_undecryptable_sections = contents.invalid_sections_count() as u8;
        let legible_sections = contents.into_sections();
        let num_legible_sections = legible_sections.len() as u8;
        let legible_sections =
            LegibleV1Sections::allocate_with_contents(legible_sections.into_vec())?;
        Ok(Self { num_undecryptable_sections, num_legible_sections, legible_sections })
    }
}

/// Internal, Rust-side implementation of a listing of legible sections
/// in a deserialized V1 advertisement
pub struct LegibleV1SectionsInternals {
    sections: Vec<DeserializedV1SectionInternals>,
}

impl LegibleV1SectionsInternals {
    fn get_section_internals(
        &self,
        legible_section_index: u8,
    ) -> Option<&DeserializedV1SectionInternals> {
        self.sections.get(legible_section_index as usize)
    }
    fn get_section(
        &self,
        legible_sections_handle: LegibleV1Sections,
        legible_section_index: u8,
    ) -> GetV1SectionResult {
        match self.get_section_internals(legible_section_index) {
            Some(section_ref) => {
                // Determine whether the section is plaintext
                // or decrypted to report back to the caller,
                // and also determine the number of contained DEs.
                let num_des = section_ref.num_des();
                let identity_tag = section_ref.identity_kind();
                GetV1SectionResult::Success(DeserializedV1Section {
                    legible_sections_handle,
                    legible_section_index,
                    num_des,
                    identity_tag,
                })
            }
            None => GetV1SectionResult::Error,
        }
    }
}

impl<'adv>
    TryFrom<
        Vec<
            V1DeserializedSection<
                'adv,
                np_adv::credential::matched::ReferencedMatchedCredential<'adv, MatchedCredential>,
            >,
        >,
    > for LegibleV1SectionsInternals
{
    type Error = DataElementParseError;

    fn try_from(
        contents: Vec<
            V1DeserializedSection<
                'adv,
                np_adv::credential::matched::ReferencedMatchedCredential<'adv, MatchedCredential>,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let sections = contents
            .into_iter()
            .map(DeserializedV1SectionInternals::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { sections })
    }
}

/// A `#[repr(C)]` handle to a value of type `LegibleV1SectionsInternals`
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LegibleV1Sections {
    handle_id: u64,
}

declare_handle_map!(
    crate::common::default_handle_map_dimensions(),
    LegibleV1Sections,
    LegibleV1SectionsInternals
);

impl LocksLongerThan<LegibleV1Sections> for CredentialBook {}

impl LegibleV1Sections {
    pub(crate) fn allocate_with_contents(
        contents: Vec<
            V1DeserializedSection<
                np_adv::credential::matched::ReferencedMatchedCredential<MatchedCredential>,
            >,
        >,
    ) -> Result<Self, DeserializeAdvertisementError> {
        let section = LegibleV1SectionsInternals::try_from(contents)
            .map_err(|_| DeserializeAdvertisementError)?;
        Self::allocate(move || section).map_err(|e| e.into())
    }

    /// Gets the legible section with the given index (which is bounded in
    /// `0..self.num_legible_sections()`). This function uses this handle but does not take
    /// ownership of it.
    pub fn get_section(&self, legible_section_index: u8) -> GetV1SectionResult {
        match self.get() {
            Ok(sections_read_guard) => {
                sections_read_guard.get_section(*self, legible_section_index)
            }
            Err(_) => GetV1SectionResult::Error,
        }
    }

    /// Get a data element by section index and de index. Similar to `get_section().get_generic_de()` but
    /// will only lock the HandleMap once. This function uses this handle but does not take
    /// ownership of it.
    pub fn get_section_de(&self, legible_section_index: u8, de_index: u8) -> GetV1DEResult {
        let Ok(sections) = self.get() else {
            return GetV1DEResult::Error;
        };
        let Some(section) = sections.get_section_internals(legible_section_index) else {
            return GetV1DEResult::Error;
        };
        section.get_generic_de(de_index)
    }

    /// Gets identity details for the legible section at the given index. Similar to
    /// `get_section().get_identity_details()` but will only lock the HandleMap once. This function
    /// uses this handle but does not take ownership of it.
    pub fn get_section_identity_details(
        &self,
        legible_section_index: u8,
    ) -> GetV1IdentityDetailsResult {
        let Ok(sections) = self.get() else {
            return GetV1IdentityDetailsResult::Error;
        };
        let Some(section) = sections.get_section_internals(legible_section_index) else {
            return GetV1IdentityDetailsResult::Error;
        };
        section.get_identity_details()
    }

    /// Decrypts metadata for the legible section at the given index. Similar to
    /// `get_section().decrypt_metadata()` but will only lock the HandleMap once. This function
    /// uses this handle but does not take ownership of it. The caller is given owenership of the
    /// handle in the result if present.
    pub fn decrypt_section_metadata(&self, legible_section_index: u8) -> DecryptMetadataResult {
        let Ok(sections) = self.get() else {
            return DecryptMetadataResult::Error;
        };
        let Some(section) = sections.get_section_internals(legible_section_index) else {
            return DecryptMetadataResult::Error;
        };
        section.decrypt_metadata()
    }
}

/// Discriminant for `GetV1SectionResult`
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum GetV1SectionResultKind {
    /// The attempt to get the section failed,
    /// possibly due to the section index being
    /// out-of-bounds or due to the underlying
    /// advertisement having already been deallocated.
    Error = 0,
    /// The attempt to get the section succeeded.
    /// The wrapped section may be obtained via
    /// `GetV1SectionResult#into_success`.
    Success = 1,
}

/// The result of attempting to get a particular V1 section
/// from its' index within the list of legible sections
/// via `DeserializedV1Advertisement::get_section`.
#[repr(C)]
#[allow(missing_docs)]
pub enum GetV1SectionResult {
    Error,
    Success(DeserializedV1Section),
}

impl FfiEnum for GetV1SectionResult {
    type Kind = GetV1SectionResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            GetV1SectionResult::Error => GetV1SectionResultKind::Error,
            GetV1SectionResult::Success(_) => GetV1SectionResultKind::Success,
        }
    }
}

impl GetV1SectionResult {
    declare_enum_cast! {into_success, Success, DeserializedV1Section}
}

/// Discriminant for `GetV1DE16ByteSaltResult`.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum GetV1DE16ByteSaltResultKind {
    /// The attempt to get the derived salt failed, possibly
    /// because the passed DE type code was forbidden (0xFFFFFFFF),
    /// or because there was no salt included for the
    /// referenced advertisement section (i.e: it was
    /// a public advertisement section, or it was deallocated.)
    Error = 0,
    /// A 16-byte salt for the given DE type-code was successfully
    /// derived.
    Success = 1,
}

/// The result of attempting to get a derived 16-byte salt
/// for a given DE within a section.
#[repr(C)]
#[allow(missing_docs)]
pub enum GetV1DE16ByteSaltResult {
    Error,
    Success(FixedSizeArray<16>),
}

impl GetV1DE16ByteSaltResult {
    declare_enum_cast! {into_success, Success, FixedSizeArray<16>}
}

impl FfiEnum for GetV1DE16ByteSaltResult {
    type Kind = GetV1DE16ByteSaltResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            GetV1DE16ByteSaltResult::Error => GetV1DE16ByteSaltResultKind::Error,
            GetV1DE16ByteSaltResult::Success(_) => GetV1DE16ByteSaltResultKind::Success,
        }
    }
}

/// The internal FFI-friendly representation of a deserialized v1 section
pub struct DeserializedV1SectionInternals {
    des: Vec<GenericV1DataElement>,
    identity: Option<DeserializedV1IdentityInternals>,
}

impl DeserializedV1SectionInternals {
    /// Gets the number of data-elements in this section.
    fn num_des(&self) -> u8 {
        self.des.len() as u8
    }

    /// Gets the enum tag of the identity used for this section.
    fn identity_kind(&self) -> DeserializedV1IdentityKind {
        if self.identity.is_some() {
            DeserializedV1IdentityKind::Decrypted
        } else {
            DeserializedV1IdentityKind::Plaintext
        }
    }

    /// Attempts to get the DE with the given index in this section.
    fn get_generic_de(&self, index: u8) -> GetV1DEResult {
        match self.des.get(index as usize) {
            Some(de) => GetV1DEResult::Success(de.clone()),
            None => GetV1DEResult::Error,
        }
    }

    /// Attempts to get the directly-transmissible details about
    /// the deserialized V1 identity for this section. Does
    /// not include decrypted metadata bytes nor the section salt.
    pub(crate) fn get_identity_details(&self) -> GetV1IdentityDetailsResult {
        match &self.identity {
            Some(identity) => GetV1IdentityDetailsResult::Success(identity.details()),
            None => GetV1IdentityDetailsResult::Error,
        }
    }

    /// Attempts to decrypt the metadata for the matched
    /// credential for this V1 section (if any).
    pub(crate) fn decrypt_metadata(&self) -> DecryptMetadataResult {
        match &self.identity {
            None => DecryptMetadataResult::Error,
            Some(identity) => match identity.decrypt_metadata() {
                None => DecryptMetadataResult::Error,
                Some(metadata) => allocate_decrypted_metadata_handle(metadata),
            },
        }
    }

    /// Attempts to derive a 16-byte DE salt for a DE in this section
    /// with the given DE type-code. This operation may fail if the
    /// passed type-code is the forbidden code 0xFFFFFFFF or if the section
    /// is leveraging a public identity, and hence, doesn't have
    /// an associated salt.
    pub(crate) fn derive_16_byte_salt_for_de_type<C: CryptoProvider>(
        &self,
        de_type: V1DEType,
    ) -> GetV1DE16ByteSaltResult {
        self.identity
            .as_ref()
            .and_then(|x| x.derive_16_byte_salt_for_de_type::<C>(de_type))
            .map_or(GetV1DE16ByteSaltResult::Error, GetV1DE16ByteSaltResult::Success)
    }
}

impl<'adv>
    TryFrom<
        V1DeserializedSection<
            'adv,
            np_adv::credential::matched::ReferencedMatchedCredential<'adv, MatchedCredential>,
        >,
    > for DeserializedV1SectionInternals
{
    type Error = DataElementParseError;

    fn try_from(
        section: V1DeserializedSection<
            np_adv::credential::matched::ReferencedMatchedCredential<'adv, MatchedCredential>,
        >,
    ) -> Result<Self, Self::Error> {
        use np_adv::extended::deserialize::Section;
        match section {
            V1DeserializedSection::Plaintext(section) => {
                let des = section
                    .iter_data_elements()
                    .map(|r| r.map(GenericV1DataElement::from))
                    .collect::<Result<Vec<_>, _>>()?;
                let identity = None;
                Ok(Self { des, identity })
            }
            V1DeserializedSection::Decrypted(with_matched) => {
                let section = with_matched.contents();
                let des = section
                    .iter_data_elements()
                    .map(|r| r.map(GenericV1DataElement::from))
                    .collect::<Result<Vec<_>, _>>()?;

                let verification_mode = section.verification_mode();
                let salt = *section.salt();

                let match_data = with_matched.clone_match_data();
                let match_data = match_data.map(|x| *x.identity_token());

                let identity =
                    Some(DeserializedV1IdentityInternals::new(verification_mode, salt, match_data));
                Ok(Self { des, identity })
            }
        }
    }
}
/// Discriminant for `DeserializedV1Identity`.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum DeserializedV1IdentityKind {
    /// The deserialized v1 identity was plaintext
    Plaintext = 0,
    /// The deserialized v1 identity corresponded
    /// to some kind of decrypted identity.
    Decrypted = 1,
}

/// Internals for the representation of a decrypted
/// V1 section identity.
pub(crate) struct DeserializedV1IdentityInternals {
    /// The details about the identity, suitable
    /// for direct communication over FFI
    details: DeserializedV1IdentityDetails,
    /// The metadata key, together with the matched
    /// credential and enough information to decrypt
    /// the credential metadata, if desired.
    match_data: WithMatchedCredential<MatchedCredential, np_adv::extended::V1IdentityToken>,
    /// The 16-byte section salt
    salt: MultiSalt,
}

impl DeserializedV1IdentityInternals {
    pub(crate) fn new(
        verification_mode: np_adv::extended::deserialize::VerificationMode,
        salt: MultiSalt,
        match_data: WithMatchedCredential<MatchedCredential, np_adv::extended::V1IdentityToken>,
    ) -> Self {
        let cred_id = match_data.matched_credential().id();
        let identity_token = match_data.contents();
        let key_seed = match_data.matched_credential().key_seed();
        let details = DeserializedV1IdentityDetails::new(
            cred_id,
            verification_mode,
            *identity_token,
            key_seed,
        );
        Self { details, match_data, salt }
    }
    /// Gets the directly-transmissible details about
    /// this deserialized V1 identity. Does not include
    /// decrypted metadata bytes nor the section salt.
    pub(crate) fn details(&self) -> DeserializedV1IdentityDetails {
        self.details
    }
    /// Attempts to decrypt the metadata associated
    /// with this identity.
    pub(crate) fn decrypt_metadata(&self) -> Option<Vec<u8>> {
        self.match_data.decrypt_metadata::<CryptoProviderImpl>().ok()
    }
    /// For a given DE type-code, derives a 16-byte DE salt
    /// for a DE in that position within this section.
    pub(crate) fn derive_16_byte_salt_for_de_type<C: CryptoProvider>(
        &self,
        de_type: V1DEType,
    ) -> Option<FixedSizeArray<16>> {
        let de_type = np_hkdf::v1_salt::DeType::try_from(de_type).ok()?;

        match self.salt {
            MultiSalt::Short(_) => None,
            MultiSalt::Extended(s) => {
                s.derive::<16, C>(Some(de_type).into()).map(FixedSizeArray::from)
            }
        }
    }
}

/// Discriminant for `GetV1IdentityDetailsResult`
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum GetV1IdentityDetailsResultKind {
    /// The attempt to get the identity details
    /// for the section failed, possibly
    /// due to the section being a public
    /// section, or the underlying
    /// advertisement has already been deallocated.
    Error = 0,
    /// The attempt to get the identity details succeeded.
    /// The wrapped identity details may be obtained via
    /// `GetV1IdentityDetailsResult#into_success`.
    Success = 1,
}

/// The result of attempting to get the identity details
/// for a V1 advertisement section via
/// `DeserializedV1Advertisement#get_identity_details`.
#[repr(C)]
#[allow(missing_docs)]
pub enum GetV1IdentityDetailsResult {
    Error,
    Success(DeserializedV1IdentityDetails),
}

impl FfiEnum for GetV1IdentityDetailsResult {
    type Kind = GetV1IdentityDetailsResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            GetV1IdentityDetailsResult::Error => GetV1IdentityDetailsResultKind::Error,
            GetV1IdentityDetailsResult::Success(_) => GetV1IdentityDetailsResultKind::Success,
        }
    }
}

impl GetV1IdentityDetailsResult {
    declare_enum_cast! {into_success, Success, DeserializedV1IdentityDetails}
}

/// Information about the identity which matched
/// a decrypted V1 section.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct DeserializedV1IdentityDetails {
    /// The verification mode (MIC/Signature) which
    /// was used to verify the decrypted adv contents.
    verification_mode: V1VerificationMode,
    /// The ID of the credential which
    /// matched the deserialized section.
    cred_id: i64,
    /// The 16-byte metadata key.
    identity_token: [u8; 16],
    /// The 32-byte key seed.
    key_seed: [u8; 32],
}

impl DeserializedV1IdentityDetails {
    pub(crate) fn new(
        cred_id: i64,
        verification_mode: np_adv::extended::deserialize::VerificationMode,
        identity_token: np_adv::extended::V1IdentityToken,
        key_seed: [u8; 32],
    ) -> Self {
        let verification_mode = verification_mode.into();
        Self { cred_id, verification_mode, identity_token: identity_token.into_bytes(), key_seed }
    }
    /// Returns the ID of the credential which matched the deserialized section.
    pub fn cred_id(&self) -> i64 {
        self.cred_id
    }
    /// Returns the verification mode (MIC/Signature) employed for the decrypted section.
    pub fn verification_mode(&self) -> V1VerificationMode {
        self.verification_mode
    }
    /// Returns the 16-byte section identity token.
    pub fn identity_token(&self) -> [u8; 16] {
        self.identity_token
    }

    /// Returns the 32-byte key seed used to decrypt the advertisement section.
    pub fn key_seed(&self) -> [u8; 32] {
        self.key_seed
    }
}

/// Handle to a deserialized V1 section
#[repr(C)]
pub struct DeserializedV1Section {
    legible_sections_handle: LegibleV1Sections,
    legible_section_index: u8,
    num_des: u8,
    identity_tag: DeserializedV1IdentityKind,
}

impl DeserializedV1Section {
    /// Gets the number of data elements contained in this section.
    /// Suitable as an iteration bound on `Self::get_generic_de`.
    pub fn num_des(&self) -> u8 {
        self.num_des
    }

    /// Gets the enum tag of the identity employed by this deserialized section.
    pub fn identity_kind(&self) -> DeserializedV1IdentityKind {
        self.identity_tag
    }

    /// Gets the DE with the given index in this section.
    pub fn get_generic_de(&self, de_index: u8) -> GetV1DEResult {
        self.apply_to_section_internals(
            move |section_ref| section_ref.get_generic_de(de_index),
            GetV1DEResult::Error,
        )
    }
    /// Attempts to get the details of the identity employed for the section referenced by this
    /// handle. May fail if the handle is invalid, or if the advertisement section leverages a
    /// public identity. This function does not take ownership of the handle.
    pub fn get_identity_details(&self) -> GetV1IdentityDetailsResult {
        self.apply_to_section_internals(
            DeserializedV1SectionInternals::get_identity_details,
            GetV1IdentityDetailsResult::Error,
        )
    }
    /// Attempts to decrypt the metadata for the matched credential for the V1 section referenced
    /// by this handle (if any). This uses but does not take ownership of the handle.
    pub fn decrypt_metadata(&self) -> DecryptMetadataResult {
        self.apply_to_section_internals(
            DeserializedV1SectionInternals::decrypt_metadata,
            DecryptMetadataResult::Error,
        )
    }
    /// Attempts to derive a 16-byte DE salt for a DE in this section with the given DE type-code.
    /// This operation may fail if the passed type-code is forbidden (0xFFFFFFFF) or if the section is
    /// leveraging a public identity, and hence, doesn't have an associated salt.
    pub fn derive_16_byte_salt_for_de_type(&self, de_type: V1DEType) -> GetV1DE16ByteSaltResult {
        self.apply_to_section_internals(
            move |section_ref| {
                section_ref.derive_16_byte_salt_for_de_type::<CryptoProviderImpl>(de_type)
            },
            GetV1DE16ByteSaltResult::Error,
        )
    }

    fn apply_to_section_internals<R>(
        &self,
        func: impl FnOnce(&DeserializedV1SectionInternals) -> R,
        lookup_failure_result: R,
    ) -> R {
        // TODO: Once the `FromResidual` trait is stabilized, this can be simplified.
        match self.legible_sections_handle.get() {
            Ok(legible_sections_read_guard) => {
                match legible_sections_read_guard.get_section_internals(self.legible_section_index)
                {
                    Some(section_ref) => func(section_ref),
                    None => lookup_failure_result,
                }
            }
            Err(_) => lookup_failure_result,
        }
    }
}

/// Discriminant for the `GetV1DEResult` enum.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum GetV1DEResultKind {
    /// Attempting to get the DE at the given position failed,
    /// possibly due to the index being out-of-bounds or due
    /// to the whole advertisement having been previously deallocated.
    Error = 0,
    /// Attempting to get the DE at the given position succeeded.
    /// The underlying DE may be extracted with `GetV1DEResult#into_success`.
    Success = 1,
}

/// Represents the result of the `DeserializedV1Section#get_generic_de` operation.
#[repr(C)]
#[allow(missing_docs)]
pub enum GetV1DEResult {
    Error,
    Success(GenericV1DataElement),
}

impl FfiEnum for GetV1DEResult {
    type Kind = GetV1DEResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            GetV1DEResult::Error => GetV1DEResultKind::Error,
            GetV1DEResult::Success(_) => GetV1DEResultKind::Success,
        }
    }
}

impl GetV1DEResult {
    declare_enum_cast! {into_success, Success, GenericV1DataElement}
}

impl<'a> From<np_adv::extended::deserialize::data_element::DataElement<'a>>
    for GenericV1DataElement
{
    fn from(de: np_adv::extended::deserialize::data_element::DataElement<'a>) -> Self {
        let de_type = V1DEType::from(de.de_type());
        let contents_as_slice = de.contents();
        //Guaranteed not to panic due DE size limit.
        #[allow(clippy::unwrap_used)]
        let array_view: ArrayView<u8, 127> = ArrayView::try_from_slice(contents_as_slice).unwrap();
        let payload = ByteBuffer::from_array_view(array_view);
        Self { de_type, payload }
    }
}

/// Discriminant for [`V1DataElementDeserializationResult`].
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum V1DataElementDeserializationResultKind {
    /// Deserialization was successful, in that the data element
    /// was either recognized as a well-established DE, or that
    /// the type code of the data element wasn't recognized, and
    /// so the data element was passed along as a "generic" DE.
    Success = 0,
    /// There was a Cast ID DE, but its contents
    /// were the wrong length for such an ID.
    CastIdWrongLength = 1,
    /// There was an Actions DE, but its contents
    /// could not be interpreted.
    ActionsMalformed = 2,
    /// There was a context sync sequence number DE,
    /// but the payload wasn't exactly 1 byte.
    ContextSyncSeqNumWrongLength = 3,
    /// There was a context sync sequence number DE,
    /// but the sequence number was not in 0-15.
    ContextSyncSeqNumOutOfBounds = 4,
    /// There was a deduplication hint DE, but
    /// the payload was not exactly 8 bytes.
    DedupHintWrongLength = 5,
    /// There was a device type DE, but
    /// the payload was not exactly 1 byte.
    DeviceTypeWrongLength = 6,
    /// There was a Tx Power DE, but
    /// the value was not in the
    /// accepted range of -20dBm-100dBm.
    TxPowerOutOfBounds = 7,
    /// There was a Tx Power DE, but the
    /// payload did not occupy exactly 1 byte.
    TxPowerWrongLength = 8,
    /// There was a Connectivity Info DE, but
    /// the ordering, uniqueness, or non-emptiness
    /// of the components was violated.
    ConnectivityInfoMalformed = 9,
    /// There was a Connectivity Info DE, but
    /// one of the components was malformed.
    ConnectivityInfoMalformedComponent = 10,
    /// There was a Media Deduplication ID DE, but its contents
    /// were the wrong length for such an ID.
    MediaDeduplicationIdWrongLength = 11,
    /// There was a Capabilities DE, but the payload was malformed.
    CapabilitiesMalformed = 12,
    /// There was a Requirements DE, but the payload was malformed.
    RequirementsMalformed = 13,
}

/// Representation of the result of attempting to deserialize
/// a V1 data element from a generic DE to a more structured
/// deserialized representation.
///
/// Only the general class of error is appropriately
/// represented by this enum. If you want more descriptive
/// errors, consider doing your own custom DE deserialization
/// using the plugin architecture and/or resort to more directly
/// leveraging the Rust crate `np_adv`.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
#[allow(missing_docs, clippy::large_enum_variant)]
pub enum V1DataElementDeserializationResult {
    Success(V1DataElement),
    CastIdWrongLength,
    ActionsMalformed,
    ContextSyncSeqNumWrongLength,
    ContextSyncSeqNumOutOfBounds,
    DedupHintWrongLength,
    DeviceTypeWrongLength,
    TxPowerOutOfBounds,
    TxPowerWrongLength,
    ConnectivityInfoMalformed,
    /// There was a Connectivity Info DE, but the component
    /// of the specified index within the DE was malformed.
    ConnectivityInfoMalformedComponent(u8),
    MediaDeduplicationIdWrongLength,
    CapabilitiesMalformed,
    RequirementsMalformed,
}

impl FfiEnum for V1DataElementDeserializationResult {
    type Kind = V1DataElementDeserializationResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Success(_) => V1DataElementDeserializationResultKind::Success,
            Self::CastIdWrongLength => V1DataElementDeserializationResultKind::CastIdWrongLength,
            Self::ActionsMalformed => V1DataElementDeserializationResultKind::ActionsMalformed,
            Self::ContextSyncSeqNumWrongLength => {
                V1DataElementDeserializationResultKind::ContextSyncSeqNumWrongLength
            }
            Self::ContextSyncSeqNumOutOfBounds => {
                V1DataElementDeserializationResultKind::ContextSyncSeqNumOutOfBounds
            }
            Self::DedupHintWrongLength => {
                V1DataElementDeserializationResultKind::DedupHintWrongLength
            }
            Self::DeviceTypeWrongLength => {
                V1DataElementDeserializationResultKind::DeviceTypeWrongLength
            }
            Self::TxPowerOutOfBounds => V1DataElementDeserializationResultKind::TxPowerOutOfBounds,
            Self::TxPowerWrongLength => V1DataElementDeserializationResultKind::TxPowerWrongLength,
            Self::ConnectivityInfoMalformed => {
                V1DataElementDeserializationResultKind::ConnectivityInfoMalformed
            }
            Self::ConnectivityInfoMalformedComponent(_) => {
                V1DataElementDeserializationResultKind::ConnectivityInfoMalformedComponent
            }
            Self::MediaDeduplicationIdWrongLength => {
                V1DataElementDeserializationResultKind::MediaDeduplicationIdWrongLength
            }
            Self::CapabilitiesMalformed => {
                V1DataElementDeserializationResultKind::CapabilitiesMalformed
            }
            Self::RequirementsMalformed => {
                V1DataElementDeserializationResultKind::RequirementsMalformed
            }
        }
    }
}

impl V1DataElementDeserializationResult {
    declare_enum_cast! {into_success, Success, V1DataElement}
    declare_enum_cast! {into_malformed_component_index, ConnectivityInfoMalformedComponent, u8}
}

impl<'a> TryFrom<&'a GenericV1DataElement>
    for np_adv::extended::deserialize::data_element::ProtoDataElement<'a>
{
    type Error = InvalidStackDataStructure;
    fn try_from(de: &'a GenericV1DataElement) -> Result<Self, Self::Error> {
        let contents = de.payload.as_slice()?;
        let de_type = de.de_type.try_into().map_err(|_| InvalidStackDataStructure)?;
        Ok(Self { contents, de_type })
    }
}

impl GenericV1DataElement {
    /// Attempts to deserialize the contents of this generic V1 data element
    /// to one of the predefined DE definitions of [`V1DataElement`], returning
    /// an error if the format of the DE payload didn't match what was expected
    /// for the type, and passing along the DE as a generic DE if the type-code
    /// did not match one of the predefined DEs.
    pub fn try_deserialize(
        self,
    ) -> Result<V1DataElementDeserializationResult, InvalidStackDataStructure> {
        use np_adv::extended::data_elements::DeserializedGoogleDE;
        use np_adv::extended::deserialize::data_element::DataElementDeserializationResult;

        let proto_de = (&self).try_into()?;
        // NOTE: Keeping the salt as [`None`] only works because our predefined
        // data elements all have no dependence on derived DE salts for deserialization.
        let de =
            np_adv::extended::deserialize::data_element::DataElement::from_proto_de(proto_de, None);
        Ok(match DeserializedGoogleDE::try_deserialize(de) {
            Ok(DeserializedGoogleDE::CastId(Ok(cast_id))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::CastId(cast_id.into()))
            }
            Ok(DeserializedGoogleDE::CastId(Err(
                np_adv::extended::data_elements::CastIdDeserializationError::WrongLength,
            ))) => V1DataElementDeserializationResult::CastIdWrongLength,
            Ok(DeserializedGoogleDE::Actions(Ok(actions))) => match actions.action_ids_bitmap() {
                Ok(bitmap) => V1DataElementDeserializationResult::Success(V1DataElement::Actions(
                    V1ActionsBitmap { bitmap },
                )),
                Err(_) => V1DataElementDeserializationResult::ActionsMalformed,
            },
            Ok(DeserializedGoogleDE::Actions(Err(_))) => {
                V1DataElementDeserializationResult::ActionsMalformed
            }
            Ok(DeserializedGoogleDE::TxPower(Ok(tx_power))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::TxPower(tx_power.into()))
            }
            Ok(DeserializedGoogleDE::TxPower(Err(
                np_adv::extended::data_elements::TxPowerMalformed::OutOfBounds,
            ))) => V1DataElementDeserializationResult::TxPowerOutOfBounds,
            Ok(DeserializedGoogleDE::TxPower(Err(
                np_adv::extended::data_elements::TxPowerMalformed::WrongLength,
            ))) => V1DataElementDeserializationResult::TxPowerWrongLength,
            Ok(DeserializedGoogleDE::ContextSyncSeqNum(Ok(context_sync_seq_num))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::ContextSyncSeqNum(
                    context_sync_seq_num.into(),
                ))
            }
            Ok(DeserializedGoogleDE::ContextSyncSeqNum(Err(
                np_adv::extended::data_elements::ContextSyncSeqNumMalformed::OutOfBounds,
            ))) => V1DataElementDeserializationResult::ContextSyncSeqNumOutOfBounds,
            Ok(DeserializedGoogleDE::ContextSyncSeqNum(Err(
                np_adv::extended::data_elements::ContextSyncSeqNumMalformed::WrongLength,
            ))) => V1DataElementDeserializationResult::ContextSyncSeqNumWrongLength,
            Ok(DeserializedGoogleDE::DedupHint(Ok(dedup_hint))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::DedupHint(
                    dedup_hint.into(),
                ))
            }
            Ok(DeserializedGoogleDE::DedupHint(Err(
                np_adv::extended::data_elements::DeduplicationHintMalformed::WrongLength,
            ))) => V1DataElementDeserializationResult::DedupHintWrongLength,
            Ok(DeserializedGoogleDE::DeviceType(Ok(device_type))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::DeviceType(
                    device_type.into(),
                ))
            }
            Ok(DeserializedGoogleDE::DeviceType(Err(
                np_adv::extended::data_elements::DeviceTypeDeserializationError::WrongLength,
            ))) => V1DataElementDeserializationResult::DeviceTypeWrongLength,
            Ok(DeserializedGoogleDE::ConnectivityInfo(Ok(connectivity_info))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::ConnectivityInfo(
                    connectivity_info.into(),
                ))
            },
            Ok(DeserializedGoogleDE::ConnectivityInfo(Err(
                np_adv::extended::data_elements::ConnectivityInfoDeserializationError::ComponentParseFailure(component_index)
            ))) => V1DataElementDeserializationResult::ConnectivityInfoMalformedComponent(component_index),
            Ok(DeserializedGoogleDE::ConnectivityInfo(Err(_)))
                => V1DataElementDeserializationResult::ConnectivityInfoMalformed,
            Ok(DeserializedGoogleDE::MediaDeduplicationId(Ok(media_deduplication_id))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::MediaDeduplicationId(media_deduplication_id.into()))
            }
            Ok(DeserializedGoogleDE::MediaDeduplicationId(Err(
                np_adv::extended::data_elements::MediaDeduplicationIdDeserializationError::WrongLength,
            ))) => V1DataElementDeserializationResult::MediaDeduplicationIdWrongLength,
            Ok(DeserializedGoogleDE::Capabilities(Ok(capabilities))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::Capabilities(
                    capabilities.into(),
                ))
            }
            Ok(DeserializedGoogleDE::Capabilities(Err(_))) => V1DataElementDeserializationResult::CapabilitiesMalformed,
            Ok(DeserializedGoogleDE::Requirements(Ok(capabilities))) => {
                V1DataElementDeserializationResult::Success(V1DataElement::Requirements(
                    capabilities.into(),
                ))
            }
            Ok(DeserializedGoogleDE::Requirements(Err(_))) => V1DataElementDeserializationResult::RequirementsMalformed,
            Err(_) => V1DataElementDeserializationResult::Success(V1DataElement::Generic(self)),
        })
    }
}

impl<'a> From<np_adv::extended::data_elements::CastIdDataElement<'a>> for CastId {
    fn from(de: np_adv::extended::data_elements::CastIdDataElement<'a>) -> Self {
        Self { bytes: *de.get_id_as_bytes() }
    }
}

impl<'a> From<np_adv::extended::data_elements::MediaDeduplicationIdDataElement<'a>>
    for MediaDeduplicationId
{
    fn from(de: np_adv::extended::data_elements::MediaDeduplicationIdDataElement<'a>) -> Self {
        Self { bytes: *de.get_id_as_bytes() }
    }
}

impl From<np_adv::extended::data_elements::TxPowerDataElement> for TxPower {
    fn from(de: np_adv::extended::data_elements::TxPowerDataElement) -> Self {
        let tx_power: np_adv::shared_data::TxPower = de.into();
        tx_power.into()
    }
}

impl From<np_adv::extended::data_elements::ContextSyncSeqNumDataElement> for ContextSyncSeqNum {
    fn from(de: np_adv::extended::data_elements::ContextSyncSeqNumDataElement) -> Self {
        let num: np_adv::shared_data::ContextSyncSeqNum = de.into();
        num.into()
    }
}

impl From<np_adv::extended::data_elements::DeduplicationHintDataElement> for DedupHint {
    fn from(de: np_adv::extended::data_elements::DeduplicationHintDataElement) -> Self {
        Self { bytes: de.as_bytes() }
    }
}

impl From<np_adv::extended::data_elements::DeviceTypeDataElement> for DeviceType {
    fn from(de: np_adv::extended::data_elements::DeviceTypeDataElement) -> Self {
        let device_type = de.0;
        Self { device_type }
    }
}

impl From<np_adv::extended::data_elements::ConnectivityInfoDataElement> for ConnectivityInfo {
    fn from(de: np_adv::extended::data_elements::ConnectivityInfoDataElement) -> Self {
        let ble_info = de.ble_info.into();
        let wifi_lan_info = de.wifi_lan_info.into();
        Self { ble_info, wifi_lan_info }
    }
}

impl From<np_adv::extended::data_elements::BleConnectivityInfo> for BleConnectivityInfo {
    fn from(info: np_adv::extended::data_elements::BleConnectivityInfo) -> Self {
        let mac_address = info.mac_address.into();
        let gatt_service_identifier = match info.gatt_service_identifier {
            None => ByteBuffer::default(),
            Some(x) => ByteBuffer::from_array_vec(x),
        };
        let psm = info.psm.into();
        let device_token = info.device_token.into();
        Self { mac_address, gatt_service_identifier, psm, device_token }
    }
}

impl From<np_adv::extended::data_elements::WifiLanConnectivityInfo> for WifiLanConnectivityInfo {
    fn from(info: np_adv::extended::data_elements::WifiLanConnectivityInfo) -> Self {
        let ip = info.ip.into();
        let port = info.port.map(|x| x.to_be_bytes()).into();
        let bssid = info.bssid.into();
        Self { ip, port, bssid }
    }
}

impl From<Option<core::net::IpAddr>> for OptionalIpAddress {
    fn from(maybe_addr: Option<core::net::IpAddr>) -> Self {
        match maybe_addr {
            None => Self::NotPresent,
            Some(core::net::IpAddr::V4(x)) => Self::IPv4(x.octets().into()),
            Some(core::net::IpAddr::V6(x)) => Self::IPv6(x.octets().into()),
        }
    }
}

impl From<np_adv::extended::data_elements::CapabilitiesDataElement> for Capabilities {
    fn from(de: np_adv::extended::data_elements::CapabilitiesDataElement) -> Self {
        let capabilities = de.into();
        Self { capabilities }
    }
}

impl From<np_adv::extended::data_elements::RequirementsDataElement> for Requirements {
    fn from(de: np_adv::extended::data_elements::RequirementsDataElement) -> Self {
        let requirements = de.into();
        Self { requirements }
    }
}
