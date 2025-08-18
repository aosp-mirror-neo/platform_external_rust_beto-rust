// Copyright 2023 Google LLC
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

//! NP Rust FFI structures and methods for V1 advertisement serialization.

use crate::common::*;
use crate::credentials::V1BroadcastCredential;
use crate::utils::FfiEnum;
use crate::v1::*;
use crate::CryptoProviderImpl;
use handle_map::{declare_handle_map, HandleLike, HandleMapFullError};
use np_adv::extended::serialize::{ProvidesDEType, WriteDataElement};

#[cfg(feature = "testing")]
use np_adv::extended::salt::MultiSalt;

/// A handle to a builder for V1 advertisements.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct V1AdvertisementBuilder {
    handle_id: u64,
}

declare_handle_map!(
    crate::common::default_handle_map_dimensions(),
    V1AdvertisementBuilder,
    V1AdvertisementBuilderInternals
);

impl V1AdvertisementBuilder {
    /// Attempts to create a builder for a new public section within this advertisement, returning
    /// an owned handle to the newly-created section builder if successful.
    ///
    /// This method may fail if there is another currently-active section builder for the same
    /// advertisement builder, if the kind of section being added does not match the advertisement
    /// type (public/encrypted), or if the section would not manage to fit within the enclosing
    /// advertisement.
    pub fn public_section_builder(&self) -> CreateV1SectionBuilderResult {
        self.section_builder_internals(|internals| internals.public_section_builder())
    }

    /// Attempts to create a builder for a new encrypted section within this advertisement,
    /// returning an owned handle to the newly-created section builder if successful.
    ///
    /// The identity details for the new section builder may be specified
    /// via providing the broadcast credential data, the kind of encrypted
    /// identity being broadcast (private/trusted/provisioned), and the
    /// verification mode to be used for the encrypted section.
    ///
    /// This method may fail if there is another currently-active
    /// section builder for the same advertisement builder, if the
    /// kind of section being added does not match the advertisement
    /// type (public/encrypted), or if the section would not manage
    /// to fit within the enclosing advertisement.
    pub fn encrypted_section_builder(
        &self,
        broadcast_cred: V1BroadcastCredential,
        verification_mode: V1VerificationMode,
    ) -> CreateV1SectionBuilderResult {
        if cfg!(feature = "nocrypto") {
            return CreateV1SectionBuilderResult::NoSpaceLeft;
        }
        self.section_builder_internals(move |internals| {
            internals.encrypted_section_builder(broadcast_cred, verification_mode)
        })
    }

    /// Attempts to create a builder for a new encrypted section within this advertisement,
    /// returning an owned handle to the newly-created section builder if successful.
    ///
    /// The identity details for the new section builder may be specified
    /// via providing the broadcast credential data, the kind of encrypted
    /// identity being broadcast (private/trusted/provisioned).
    ///
    ///
    /// The section will be encrypted with the MIC verification mode using the given salt.
    ///
    /// This method may fail if there is another currently-active
    /// section builder for the same advertisement builder, if the
    /// kind of section being added does not match the advertisement
    /// type (public/encrypted), or if the section would not manage
    /// to fit within the enclosing advertisement.
    #[cfg(feature = "testing")]
    pub fn mic_custom_salt_section_builder(
        &self,
        broadcast_cred: V1BroadcastCredential,
        salt: MultiSalt,
    ) -> CreateV1SectionBuilderResult {
        self.section_builder_internals(move |internals| {
            internals.mic_custom_salt_section_builder(broadcast_cred, salt)
        })
    }

    /// Attempts to serialize the contents of the advertisement builder behind this handle to
    /// bytes. Assuming that the handle is valid, this operation will always take ownership of the
    /// handle and result in the contents behind the advertisement builder handle being
    /// deallocated.
    pub fn into_advertisement(self) -> SerializeV1AdvertisementResult {
        match self.deallocate() {
            Ok(adv_builder) => adv_builder.into_advertisement(),
            Err(_) => SerializeV1AdvertisementResult::InvalidAdvertisementBuilderHandle,
        }
    }

    fn section_builder_internals(
        &self,
        builder_supplier: impl FnOnce(
            &mut V1AdvertisementBuilderInternals,
        ) -> Result<usize, SectionBuilderError>,
    ) -> CreateV1SectionBuilderResult {
        match self.get_mut() {
            Ok(mut adv_builder_write_guard) => {
                match builder_supplier(&mut adv_builder_write_guard) {
                    Ok(section_index) => CreateV1SectionBuilderResult::Success(V1SectionBuilder {
                        adv_builder: *self,
                        section_index: section_index as u8,
                    }),
                    Err(e) => e.into(),
                }
            }
            Err(_) => CreateV1SectionBuilderResult::InvalidAdvertisementBuilderHandle,
        }
    }
}

/// Discriminant for `CreateV1AdvertisementBuilderResult`
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CreateV1AdvertisementBuilderResultKind {
    /// The attempt to create a new advertisement builder
    /// failed since there are no more available
    /// slots for V1 advertisement builders in their handle-map.
    NoSpaceLeft = 0,
    /// The attempt succeeded. The wrapped advertisement builder
    /// may be obtained via
    /// `CreateV1AdvertisementBuilderResult#into_success`.
    Success = 1,
}

/// The result of attempting to create a new V1 advertisement builder.
#[repr(C)]
#[allow(missing_docs)]
pub enum CreateV1AdvertisementBuilderResult {
    NoSpaceLeft,
    Success(V1AdvertisementBuilder),
}

impl From<Result<V1AdvertisementBuilder, HandleMapFullError>>
    for CreateV1AdvertisementBuilderResult
{
    fn from(result: Result<V1AdvertisementBuilder, HandleMapFullError>) -> Self {
        match result {
            Ok(builder) => CreateV1AdvertisementBuilderResult::Success(builder),
            Err(_) => CreateV1AdvertisementBuilderResult::NoSpaceLeft,
        }
    }
}

impl FfiEnum for CreateV1AdvertisementBuilderResult {
    type Kind = CreateV1AdvertisementBuilderResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            CreateV1AdvertisementBuilderResult::NoSpaceLeft => {
                CreateV1AdvertisementBuilderResultKind::NoSpaceLeft
            }
            CreateV1AdvertisementBuilderResult::Success(_) => {
                CreateV1AdvertisementBuilderResultKind::Success
            }
        }
    }
}

impl CreateV1AdvertisementBuilderResult {
    declare_enum_cast! {into_success, Success, V1AdvertisementBuilder }
}

/// Creates a new V1 advertisement builder for the given advertisement
/// kind (public/encrypted).
pub fn create_v1_advertisement_builder() -> CreateV1AdvertisementBuilderResult {
    V1AdvertisementBuilder::allocate(V1AdvertisementBuilderInternals::new).into()
}

pub(crate) enum V1AdvertisementBuilderState {
    /// Internal state for when we have an active advertisement
    /// builder, but no currently-active section builder.
    Advertisement(np_adv_dynamic::extended::BoxedAdvBuilder),
    /// Internal state for when we have both an active advertisement
    /// builder and an active section builder.
    Section(np_adv_dynamic::extended::BoxedSectionBuilder<np_adv::extended::serialize::AdvBuilder>),
}

/// Internal version of errors which may be raised when
/// attempting to derive a new section builder from an
/// advertisement builder.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SectionBuilderError {
    /// We're currently in the middle of building a section.
    UnclosedActiveSection,
    /// There isn't enough space for a new section, either
    /// because the maximum section count has been exceeded
    /// or because the advertisement is almost full, and
    /// the minimum size of a section wouldn't fit.
    NoSpaceLeft,
}

impl From<np_adv_dynamic::extended::BoxedAddSectionError> for SectionBuilderError {
    fn from(err: np_adv_dynamic::extended::BoxedAddSectionError) -> Self {
        use np_adv::extended::serialize::AddSectionError;
        use np_adv_dynamic::extended::BoxedAddSectionError;
        match err {
            BoxedAddSectionError::Underlying(AddSectionError::InsufficientAdvSpace)
            | BoxedAddSectionError::Underlying(AddSectionError::MaxSectionCountExceeded) => {
                SectionBuilderError::NoSpaceLeft
            }
        }
    }
}

/// Discriminant for `SerializeV1AdvertisementResult`.
#[repr(u8)]
pub enum SerializeV1AdvertisementResultKind {
    /// Serializing the advertisement to bytes was successful.
    Success = 0,
    /// The state of the advertisement builder was invalid
    /// for the builder to be closed for serialization, likely
    /// because there was an unclosed section builder.
    InvalidBuilderState = 1,
    /// The advertisement builder handle was invalid.
    InvalidAdvertisementBuilderHandle = 2,
}

/// The result of attempting to serialize the contents
/// of a V1 advertisement builder to raw bytes.
#[repr(C)]
#[allow(missing_docs, clippy::large_enum_variant)]
pub enum SerializeV1AdvertisementResult {
    Success(ByteBuffer<250>),
    InvalidBuilderState,
    InvalidAdvertisementBuilderHandle,
}

impl SerializeV1AdvertisementResult {
    declare_enum_cast! { into_success, Success, ByteBuffer<250> }
}

impl FfiEnum for SerializeV1AdvertisementResult {
    type Kind = SerializeV1AdvertisementResultKind;
    fn kind(&self) -> SerializeV1AdvertisementResultKind {
        match self {
            Self::Success(_) => SerializeV1AdvertisementResultKind::Success,
            Self::InvalidBuilderState => SerializeV1AdvertisementResultKind::InvalidBuilderState,
            Self::InvalidAdvertisementBuilderHandle => {
                SerializeV1AdvertisementResultKind::InvalidAdvertisementBuilderHandle
            }
        }
    }
}

/// Internal, Rust-side implementation of a V1 advertisement builder.
pub struct V1AdvertisementBuilderInternals {
    // Note: This is actually always populated from an external
    // perspective in the absence of panics. We only need
    // the `Option` in order to be able to take ownership
    // and perform a state transition when needed.
    state: Option<V1AdvertisementBuilderState>,
}

impl V1AdvertisementBuilderInternals {
    pub(crate) fn new() -> Self {
        let builder = np_adv::extended::serialize::AdvBuilder::new();
        let builder = builder.into();
        let state = Some(V1AdvertisementBuilderState::Advertisement(builder));
        Self { state }
    }
    /// Internals of section_builder-type routines. Upon success, yields the index
    /// of the newly-added section builder.
    pub(crate) fn section_builder_internal(
        &mut self,
        identity: np_adv_dynamic::extended::BoxedEncoder,
    ) -> Result<usize, SectionBuilderError> {
        let state = self.state.take();
        match state {
            Some(V1AdvertisementBuilderState::Advertisement(adv_builder)) => {
                match adv_builder.into_section_builder(identity) {
                    Ok(section_builder) => {
                        let section_index = section_builder.previously_added_section_count();
                        self.state = Some(V1AdvertisementBuilderState::Section(section_builder));
                        Ok(section_index)
                    }
                    Err((adv_builder, err)) => {
                        self.state = Some(V1AdvertisementBuilderState::Advertisement(adv_builder));
                        Err(err.into())
                    }
                }
            }
            x => {
                // Note: Technically, this case also would leave the `None` state
                // if we ever entered into it, but we never transition to that
                // state during normal operation.
                self.state = x;
                Err(SectionBuilderError::UnclosedActiveSection)
            }
        }
    }

    pub(crate) fn public_section_builder(&mut self) -> Result<usize, SectionBuilderError> {
        let identity = np_adv_dynamic::extended::BoxedEncoder::Unencrypted;
        self.section_builder_internal(identity)
    }
    pub(crate) fn encrypted_section_builder(
        &mut self,
        broadcast_cred: V1BroadcastCredential,
        verification_mode: V1VerificationMode,
    ) -> Result<usize, SectionBuilderError> {
        let mut rng = get_global_crypto_rng();
        let rng = rng.get_rng();
        let internal_broadcast_cred = broadcast_cred.into_internal();
        let encoder = match verification_mode {
            V1VerificationMode::Mic => {
                let encoder =
                    np_adv::extended::serialize::MicEncryptedSectionEncoder::<_>::new_wrapped_salt::<
                        CryptoProviderImpl,
                    >(rng, &internal_broadcast_cred);
                np_adv_dynamic::extended::BoxedEncoder::MicEncrypted(encoder)
            }
        };
        self.section_builder_internal(encoder)
    }

    #[cfg(feature = "testing")]
    pub(crate) fn mic_custom_salt_section_builder(
        &mut self,
        broadcast_cred: V1BroadcastCredential,
        salt: MultiSalt,
    ) -> Result<usize, SectionBuilderError> {
        let internal_broadcast_cred = broadcast_cred.into_internal();
        let encoder = np_adv::extended::serialize::MicEncryptedSectionEncoder::new_with_salt::<
            CryptoProviderImpl,
        >(salt, &internal_broadcast_cred);
        let encoder = np_adv_dynamic::extended::BoxedEncoder::MicEncrypted(encoder);
        self.section_builder_internal(encoder)
    }

    fn into_advertisement(self) -> SerializeV1AdvertisementResult {
        match self.state {
            Some(V1AdvertisementBuilderState::Advertisement(adv_builder)) => {
                let array_view = adv_builder.into_advertisement().into_array_view();
                SerializeV1AdvertisementResult::Success(ByteBuffer::from_array_view(array_view))
            }
            _ => SerializeV1AdvertisementResult::InvalidBuilderState,
        }
    }
}

/// Discriminant for `CreateV1SectionBuilderResult`
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CreateV1SectionBuilderResultKind {
    /// The attempt to create a new section builder succeeded.
    Success = 0,
    /// We're currently in the middle of building a section.
    UnclosedActiveSection = 1,
    /// The advertisement builder handle was invalid.
    InvalidAdvertisementBuilderHandle = 2,
    /// There isn't enough space for a new section, either
    /// because the maximum section count has been exceeded
    /// or because the advertisement is almost full, and
    /// the minimum size of a section wouldn't fit.
    NoSpaceLeft = 3,
}

/// The result of attempting to create a new V1 section builder.
#[repr(C)]
#[allow(missing_docs)]
pub enum CreateV1SectionBuilderResult {
    Success(V1SectionBuilder),
    UnclosedActiveSection,
    InvalidAdvertisementBuilderHandle,
    NoSpaceLeft,
}

impl FfiEnum for CreateV1SectionBuilderResult {
    type Kind = CreateV1SectionBuilderResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Success(_) => CreateV1SectionBuilderResultKind::Success,
            Self::UnclosedActiveSection => CreateV1SectionBuilderResultKind::UnclosedActiveSection,
            Self::InvalidAdvertisementBuilderHandle => {
                CreateV1SectionBuilderResultKind::InvalidAdvertisementBuilderHandle
            }
            Self::NoSpaceLeft => CreateV1SectionBuilderResultKind::NoSpaceLeft,
        }
    }
}

impl CreateV1SectionBuilderResult {
    declare_enum_cast! {into_success, Success, V1SectionBuilder}
}

impl From<SectionBuilderError> for CreateV1SectionBuilderResult {
    fn from(err: SectionBuilderError) -> Self {
        match err {
            SectionBuilderError::UnclosedActiveSection => Self::UnclosedActiveSection,
            SectionBuilderError::NoSpaceLeft => Self::NoSpaceLeft,
        }
    }
}

/// Result code for [`V1SectionBuilder#add_to_advertisement`].
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum AddV1SectionToAdvertisementResult {
    /// The section referenced by the given handle
    /// couldn't be added to the containing advertisement,
    /// possibly because the handle is invalid or the section
    /// has already been added to the containing section.
    Error = 0,
    /// The section referenced by the given handle
    /// was successfully added to the containing advertisement.
    /// After obtaining this result code, the section
    /// handle will no longer be valid.
    Success = 1,
}

/// Result code for operations adding DEs to a section builder.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum AddV1DEResult {
    /// The DE was successfully added to the section builder
    /// behind the given handle.
    Success = 0,
    /// The handle for the section builder was invalid.
    InvalidSectionHandle = 1,
    /// There was no more space left in the advertisement
    /// to fit the DE in the containing section.
    InsufficientSectionSpace = 2,
    /// The data element itself had invalid characteristics,
    /// most likely a length above 127.
    InvalidDataElement = 3,
    /// A data element with the same type-code was already
    /// present in the section.
    DuplicateDataElementTypeCode = 4,
}

/// Discriminant for `V1DE16ByteSaltResult`.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum V1DE16ByteSaltResultKind {
    /// We couldn't return a 16-byte DE salt, possibly
    /// because the handle to the section builder
    /// was invalid, or possibly because the section
    /// builder was for a public section, or possibly
    /// because the DE type code was forbidden (0xFFFFFFFF).
    Error = 0,
    /// A 16-byte DE salt was returned successfully.
    Success = 1,
}

/// The result of attempting to get the derived V1 DE
/// 16-byte salt for the given type-code from the section
/// builder behind the given handle.
#[derive(Clone)]
#[repr(C)]
#[allow(missing_docs)]
pub enum V1DE16ByteSaltResult {
    Error,
    Success(FixedSizeArray<16>),
}

impl FfiEnum for V1DE16ByteSaltResult {
    type Kind = V1DE16ByteSaltResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Error => V1DE16ByteSaltResultKind::Error,
            Self::Success(_) => V1DE16ByteSaltResultKind::Success,
        }
    }
}

impl V1DE16ByteSaltResult {
    fn new_from_de_salt(salt: Option<np_adv::extended::salt::DeSalt>) -> Self {
        match salt.and_then(|salt| salt.derive::<16, CryptoProviderImpl>()) {
            Some(salt) => V1DE16ByteSaltResult::Success(FixedSizeArray::from(salt)),
            None => V1DE16ByteSaltResult::Error,
        }
    }

    declare_enum_cast! {into_success, Success, FixedSizeArray<16> }
}

/// A handle to a builder for V1 sections. This is not a unique handle; it is the same handle as
/// the advertisement builder the section builder was originated from.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct V1SectionBuilder {
    /// The parent advertisement builder for this section
    pub adv_builder: V1AdvertisementBuilder,
    /// This section's logical index in the parent advertisement.
    /// Equal to the number of sections which have been previously
    /// added to the advertisement, but not suitable for identifying
    /// a section's position within an advertisement, since sections
    /// get re-ordered by encoding type as an advertisement is constructed.
    pub section_index: u8,
}

impl V1SectionBuilder {
    /// Attempts to add the section constructed behind this handle
    /// to a section builder to the containing advertisement it
    /// originated from.
    pub fn add_to_advertisement(self) -> AddV1SectionToAdvertisementResult {
        match self.adv_builder.get_mut() {
            Ok(mut adv_builder) => {
                let state = adv_builder.state.take();
                match state {
                    Some(V1AdvertisementBuilderState::Section(section_builder)) => {
                        // Make sure the index of the section we're trying to close
                        // matches the count of sections which were previously constructed.
                        // This is done as a way to warn developers of misuses of
                        // section builders [e.g: holding multiple section builders
                        // simultaneously] in the absence of ownership semantics.
                        let actual_section_index =
                            section_builder.previously_added_section_count() as u8;
                        if self.section_index == actual_section_index {
                            let updated_adv_builder =
                                section_builder.add_to_advertisement::<CryptoProviderImpl>();
                            adv_builder.state = Some(V1AdvertisementBuilderState::Advertisement(
                                updated_adv_builder,
                            ));
                            AddV1SectionToAdvertisementResult::Success
                        } else {
                            adv_builder.state =
                                Some(V1AdvertisementBuilderState::Section(section_builder));
                            AddV1SectionToAdvertisementResult::Error
                        }
                    }
                    x => {
                        adv_builder.state = x;
                        AddV1SectionToAdvertisementResult::Error
                    }
                }
            }
            Err(_) => AddV1SectionToAdvertisementResult::Error,
        }
    }

    /// Attempts to get the derived 16-byte V1 DE salt for the
    /// given DE type-code. May fail if this section builder
    /// handle is invalid, if the section is a public section,
    /// or if the DE type-code is forbidden (0xFFFFFFFF).
    pub fn de_salt(&self, de_type: V1DEType) -> V1DE16ByteSaltResult {
        match de_type.try_into() {
            Ok(de_type) => self.try_apply_to_internals(
                |section_builder| {
                    V1DE16ByteSaltResult::new_from_de_salt(section_builder.de_salt(de_type))
                },
                V1DE16ByteSaltResult::Error,
            ),
            Err(_) => V1DE16ByteSaltResult::Error,
        }
    }

    /// Attempts to add the given DE to the section builder behind
    /// this handle. The passed DE may have a payload of up to 127
    /// bytes, the maximum for a V1 DE.
    pub fn add_generic_de(&self, de: GenericV1DataElement) -> AddV1DEResult {
        match de.into_internal() {
            Some(generic_de) => self.add_de_internals(&generic_de),
            None => AddV1DEResult::InvalidDataElement,
        }
    }

    fn add_de_internals(
        &self,
        de: &dyn np_adv_dynamic::extended::DynWriteDataElement,
    ) -> AddV1DEResult {
        self.try_apply_to_internals(
            move |section_builder| match section_builder.add_de(de) {
                Ok(_) => AddV1DEResult::Success,
                Err(np_adv_dynamic::extended::BoxedAddDataElementError::DuplicateDataElementTypeCode) => {
                    AddV1DEResult::DuplicateDataElementTypeCode
                },
                Err(np_adv_dynamic::extended::BoxedAddDataElementError::InsufficientSpace) => {
                    AddV1DEResult::InsufficientSectionSpace
                },
                Err(np_adv_dynamic::extended::BoxedAddDataElementError::NoDerivedSalt) => {
                    // Currently, all stock DEs don't take a salt.
                    AddV1DEResult::InvalidDataElement
                },
            },
            AddV1DEResult::InvalidSectionHandle,
        )
    }

    fn try_apply_to_internals<R>(
        &self,
        func: impl FnOnce(
            &mut np_adv_dynamic::extended::BoxedSectionBuilder<
                np_adv::extended::serialize::AdvBuilder,
            >,
        ) -> R,
        invalid_handle_error: R,
    ) -> R {
        match self.adv_builder.get_mut() {
            Ok(mut adv_builder) => {
                match adv_builder.state.as_mut() {
                    Some(V1AdvertisementBuilderState::Section(ref mut section_builder)) => {
                        // Check to make sure that the section index matches, otherwise
                        // we have an invalid handle.
                        let current_section_index =
                            section_builder.previously_added_section_count() as u8;
                        if current_section_index == self.section_index {
                            func(section_builder)
                        } else {
                            invalid_handle_error
                        }
                    }
                    Some(V1AdvertisementBuilderState::Advertisement(_)) => invalid_handle_error,
                    None => invalid_handle_error,
                }
            }
            Err(_) => invalid_handle_error,
        }
    }
}

impl GenericV1DataElement {
    /// Attempts to convert this FFI-friendly DE with a byte-buffer size of 127
    /// to the internal representation of a generic DE. May fail in the case
    /// where the underlying payload byte-buffer has an invalid length above 127,
    /// or if the DE type-code is in fact invalid (0xFFFFFFFF).
    fn into_internal(self) -> Option<np_adv::extended::data_elements::GenericDataElement> {
        let de_type = np_adv::extended::de_type::DeType::try_from(self.de_type.to_u32()).ok()?;
        self.payload.as_slice().ok().and_then(move |payload_slice| {
            np_adv::extended::data_elements::GenericDataElement::try_from(de_type, payload_slice)
                .ok()
        })
    }
    /// Given a structure which implements [`WriteDataElement`] in such a way that
    /// the write operation is guaranteed success, writes the contents
    /// to create a new [`GenericV1DataElement`].
    #[allow(clippy::expect_used)]
    fn from_write_de<W: WriteDataElement<Salt = np_adv::extended::salt::Unsalted>>(de: &W) -> Self {
        let de_type = V1DEType::from(de.de_type());
        let mut payload: tinyvec::ArrayVec<[u8; 127]> = tinyvec::ArrayVec::new();
        de.write_de_contents(np_adv::extended::salt::Unsalted, &mut payload)
            .expect("Should always be able to write supported DEs to GenericV1DataElement");
        let payload = ByteBuffer::from_array_vec(payload);
        Self { de_type, payload }
    }
}

impl CastId {
    /// Serialize this Cast ID to a generic data element.
    pub fn serialize(&self) -> GenericV1DataElement {
        let de = np_adv::extended::data_elements::CastIdDataElement::from(&self.bytes);
        GenericV1DataElement::from_write_de(&de)
    }
}

impl MediaDeduplicationId {
    /// Serialize this Cast ID to a generic data element.
    pub fn serialize(&self) -> GenericV1DataElement {
        let de =
            np_adv::extended::data_elements::MediaDeduplicationIdDataElement::from(&self.bytes);
        GenericV1DataElement::from_write_de(&de)
    }
}

/// The result of attempting to serialize V1 actions
/// represented as a bitmap into a DE.
#[repr(C)]
#[allow(missing_docs)]
pub enum V1SerializeActionsBitmapResult {
    Success(GenericV1DataElement),
    NoActions,
    TooManyActions,
}

impl FfiEnum for V1SerializeActionsBitmapResult {
    type Kind = V1SerializeActionsBitmapResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Success(_) => V1SerializeActionsBitmapResultKind::Success,
            Self::NoActions => V1SerializeActionsBitmapResultKind::NoActions,
            Self::TooManyActions => V1SerializeActionsBitmapResultKind::TooManyActions,
        }
    }
}

impl V1SerializeActionsBitmapResult {
    declare_enum_cast! {into_success, Success, GenericV1DataElement }
}

/// Discriminant for `V1SerializeActionsBitmapResult`
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum V1SerializeActionsBitmapResultKind {
    /// The attempt succeeded. The wrapped data element
    /// may be obtained via
    /// `V1SerializeActionsBitmapResult#into_success`.
    Success = 0,
    /// The attempt to create a new actions DE failed
    /// because there were no actions to encode.
    NoActions = 1,
    /// The attempt to create a new actions DE failed
    /// because there were too many actions to encode.
    TooManyActions = 2,
}

impl V1ActionsBitmap {
    /// Attempts to serialize some V1 actions given in
    /// a bitmap format into a V1 DE byte-buffer. May fail if an attempt
    /// is made to encode more actions than can fit into a V1 actions DE,
    /// or if no actions were included.
    #[allow(clippy::expect_used)]
    pub fn try_serialize(&self) -> V1SerializeActionsBitmapResult {
        let maybe_actions_de =
            np_adv::extended::data_elements::ActionsDataElement::try_from_actions_bitmap(
                &self.bitmap,
            );
        match maybe_actions_de {
            Ok(actions_de) => {
                // Convert the internal DE representation to the FFI one.
                let mut payload = tinyvec::ArrayVec::<[u8; 127]>::new();
                actions_de
                    .write_de_contents(np_adv::extended::salt::Unsalted, &mut payload)
                    .expect("Actions DE -> 127 byte buffer should always succeed");
                let de_type = actions_de.de_type();
                let de_type = de_type.into();
                let payload = ByteBuffer::from_array_vec(payload);
                V1SerializeActionsBitmapResult::Success(GenericV1DataElement { de_type, payload })
            }
            Err(np_adv::extended::data_elements::ActionsDataElementError::EmptyActions) => {
                V1SerializeActionsBitmapResult::NoActions
            }
            Err(_) => V1SerializeActionsBitmapResult::TooManyActions,
        }
    }
}

impl TxPower {
    /// Attempts to serialize this Tx Power to a generic V1 data element.
    pub fn try_serialize_v1(&self) -> Result<GenericV1DataElement, InvalidStackDataStructure> {
        let tx_power = np_adv::shared_data::TxPower::try_from(self.tx_power)
            .map_err(|_| InvalidStackDataStructure)?;
        let de = np_adv::extended::data_elements::TxPowerDataElement::from(tx_power);
        Ok(GenericV1DataElement::from_write_de(&de))
    }
}

impl ContextSyncSeqNum {
    /// Attempts to serialize this Context Sync sequence number to a generic
    /// V1 data element.
    pub fn try_serialize(&self) -> Result<GenericV1DataElement, InvalidStackDataStructure> {
        let seq_num = np_adv::shared_data::ContextSyncSeqNum::try_from(*self)?;
        let de = np_adv::extended::data_elements::ContextSyncSeqNumDataElement::from(seq_num);
        Ok(GenericV1DataElement::from_write_de(&de))
    }
}

impl DedupHint {
    /// Serializes this deduplication hint to a generic V1 data element.
    pub fn serialize(&self) -> GenericV1DataElement {
        let de = np_adv::extended::data_elements::DeduplicationHintDataElement::from(self.bytes);
        GenericV1DataElement::from_write_de(&de)
    }
}

impl DeviceType {
    /// Serializes this device type to a generic V1 data element.
    pub fn serialize(&self) -> GenericV1DataElement {
        use np_adv::extended::data_elements::DeviceTypeDataElement;
        let de = DeviceTypeDataElement(self.device_type);
        GenericV1DataElement::from_write_de(&de)
    }
}

impl ConnectivityInfo {
    /// Serializes this connectivity info to a generic V1 data element.
    ///
    /// May yield [`InvalidStackDataStructure`] if the information is actually
    /// empty or if any of the constituent fields is a malformed stack data-structure.
    pub fn try_serialize(&self) -> Result<GenericV1DataElement, InvalidStackDataStructure> {
        let ble_info = self.ble_info.clone().try_into()?;
        let wifi_lan_info = self.wifi_lan_info.clone().into();
        let result = np_adv::extended::data_elements::ConnectivityInfoDataElement {
            ble_info,
            wifi_lan_info,
        };
        if result.is_some() {
            Ok(GenericV1DataElement::from_write_de(&result))
        } else {
            Err(InvalidStackDataStructure)
        }
    }
}

impl TryFrom<BleConnectivityInfo> for np_adv::extended::data_elements::BleConnectivityInfo {
    type Error = InvalidStackDataStructure;
    fn try_from(info: BleConnectivityInfo) -> Result<Self, Self::Error> {
        let mac_address = info.mac_address.into();
        let gatt_service_identifier =
            if let Ok(0) = info.gatt_service_identifier.as_slice().map(|x| x.len()) {
                None
            } else {
                Some(info.gatt_service_identifier.try_into()?)
            };
        let psm = info.psm.into();
        let device_token = info.device_token.into();
        Ok(Self { mac_address, gatt_service_identifier, psm, device_token })
    }
}

impl From<WifiLanConnectivityInfo> for np_adv::extended::data_elements::WifiLanConnectivityInfo {
    fn from(info: WifiLanConnectivityInfo) -> Self {
        let ip = info.ip.into();
        let port = info.port.into_present().map(|port| u16::from_be_bytes(port.into()));
        let bssid = info.bssid.into();
        Self { ip, port, bssid }
    }
}

impl From<OptionalIpAddress> for Option<core::net::IpAddr> {
    fn from(maybe_ip: OptionalIpAddress) -> Self {
        match maybe_ip {
            OptionalIpAddress::NotPresent => None,
            OptionalIpAddress::IPv4(x) => {
                Some(core::net::IpAddr::V4(u32::from_be_bytes(x.into()).into()))
            }
            OptionalIpAddress::IPv6(x) => {
                Some(core::net::IpAddr::V6(u128::from_be_bytes(x.into()).into()))
            }
        }
    }
}

/// The result of attempting to serialize capabilities
/// represented as a bitmap into a DE.
#[repr(C)]
#[allow(missing_docs)]
pub enum SerializeCapabilitiesBitmapResult {
    Success(GenericV1DataElement),
    ReservedBitsSet,
}

impl FfiEnum for SerializeCapabilitiesBitmapResult {
    type Kind = SerializeCapabilitiesBitmapResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Success(_) => SerializeCapabilitiesBitmapResultKind::Success,
            Self::ReservedBitsSet => SerializeCapabilitiesBitmapResultKind::ReservedBitsSet,
        }
    }
}

impl SerializeCapabilitiesBitmapResult {
    declare_enum_cast! {into_success, Success, GenericV1DataElement }
}

/// Discriminant for `SerializeCapabilitiesBitmapResult`
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SerializeCapabilitiesBitmapResultKind {
    /// The attempt succeeded. The wrapped data element
    /// may be obtained via
    /// `SerializeCapabilitiesBitmapResult#into_success`.
    Success = 0,
    /// The attempt failed because reserved bits were set.
    ReservedBitsSet = 1,
}

impl Capabilities {
    /// Attempts to serialize some capabilities given in
    /// a bitmap format into a DE byte-buffer. May fail if an attempt
    /// is made to encode capabilities into reserved bits.
    #[allow(clippy::expect_used)]
    pub fn try_serialize(&self) -> SerializeCapabilitiesBitmapResult {
        let maybe_capabilities_de =
            np_adv::extended::data_elements::CapabilitiesDataElement::try_from(self.capabilities);
        match maybe_capabilities_de {
            Ok(capabilities_de) => {
                // Convert the internal DE representation to the FFI one.
                let mut payload = tinyvec::ArrayVec::<[u8; 127]>::new();
                capabilities_de
                    .write_de_contents(np_adv::extended::salt::Unsalted, &mut payload)
                    .expect("Capabilities DE -> 1 byte buffer should always succeed");
                let de_type = capabilities_de.de_type();
                let de_type = de_type.into();
                let payload = ByteBuffer::from_array_vec(payload);
                SerializeCapabilitiesBitmapResult::Success(GenericV1DataElement {
                    de_type,
                    payload,
                })
            }
            Err(
                np_adv::extended::data_elements::CapabilityBitsetDeserializationError::ReservedBitsSet,
            ) => SerializeCapabilitiesBitmapResult::ReservedBitsSet,
            _ => unreachable!(),
        }
    }
}

/// The result of attempting to serialize requirements
/// represented as a bitmap into a DE.
#[repr(C)]
#[allow(missing_docs)]
pub enum SerializeRequirementsBitmapResult {
    Success(GenericV1DataElement),
    ReservedBitsSet,
}

impl FfiEnum for SerializeRequirementsBitmapResult {
    type Kind = SerializeRequirementsBitmapResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Success(_) => SerializeRequirementsBitmapResultKind::Success,
            Self::ReservedBitsSet => SerializeRequirementsBitmapResultKind::ReservedBitsSet,
        }
    }
}

impl SerializeRequirementsBitmapResult {
    declare_enum_cast! {into_success, Success, GenericV1DataElement }
}

/// Discriminant for `SerializeRequirementsBitmapResult`
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SerializeRequirementsBitmapResultKind {
    /// The attempt succeeded. The wrapped data element
    /// may be obtained via
    /// `SerializeRequirementsBitmapResult#into_success`.
    Success = 0,
    /// The attempt failed because reserved bits were set.
    ReservedBitsSet = 1,
}

impl Requirements {
    /// Attempts to serialize some requirements given in
    /// a bitmap format into a DE byte-buffer. May fail if an attempt
    /// is made to encode requirements into reserved bits.
    #[allow(clippy::expect_used)]
    pub fn try_serialize(&self) -> SerializeRequirementsBitmapResult {
        let maybe_requirements_de =
            np_adv::extended::data_elements::RequirementsDataElement::try_from(self.requirements);
        match maybe_requirements_de {
            Ok(requirements_de) => {
                // Convert the internal DE representation to the FFI one.
                let mut payload = tinyvec::ArrayVec::<[u8; 127]>::new();
                requirements_de
                    .write_de_contents(np_adv::extended::salt::Unsalted, &mut payload)
                    .expect("Requirements DE -> 1 byte buffer should always succeed");
                let de_type = requirements_de.de_type();
                let de_type = de_type.into();
                let payload = ByteBuffer::from_array_vec(payload);
                SerializeRequirementsBitmapResult::Success(GenericV1DataElement {
                    de_type,
                    payload,
                })
            }
            Err(
                np_adv::extended::data_elements::CapabilityBitsetDeserializationError::ReservedBitsSet,
            ) => SerializeRequirementsBitmapResult::ReservedBitsSet,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::indexing_slicing)]
    #[test]
    fn test_serialize_64_actions() {
        let mut actions_bitmap = V1ActionsBitmap { bitmap: [0u8; 256] };
        for i in 0..=7 {
            actions_bitmap.bitmap[i] = 255u8;
        }
        let result = actions_bitmap.try_serialize();
        assert_eq!(result.kind(), V1SerializeActionsBitmapResultKind::Success);
    }

    #[test]
    fn test_serialize_too_many_actions() {
        let actions_bitmap = V1ActionsBitmap { bitmap: [255u8; 256] };
        let result = actions_bitmap.try_serialize();
        assert_eq!(result.kind(), V1SerializeActionsBitmapResultKind::TooManyActions);
    }

    #[test]
    fn test_serialize_valid_capabilities() {
        let capabilities = Capabilities { capabilities: 0b0000_1111 };
        let result = capabilities.try_serialize();
        assert_eq!(result.kind(), SerializeCapabilitiesBitmapResultKind::Success);
    }

    #[test]
    fn test_serialize_capabilities_with_reserved_bits() {
        let capabilities = Capabilities { capabilities: 0b0001_0001 };
        let result = capabilities.try_serialize();
        assert_eq!(result.kind(), SerializeCapabilitiesBitmapResultKind::ReservedBitsSet);
    }

    #[test]
    fn test_serialize_valid_requirements() {
        let requirements = Requirements { requirements: 0b0000_1111 };
        let result = requirements.try_serialize();
        assert_eq!(result.kind(), SerializeRequirementsBitmapResultKind::Success);
    }

    #[test]
    fn test_serialize_requirements_with_reserved_bits() {
        let requirements = Requirements { requirements: 0b0001_0001 };
        let result = requirements.try_serialize();
        assert_eq!(result.kind(), SerializeRequirementsBitmapResultKind::ReservedBitsSet);
    }

    #[cfg(not(feature = "nocrypto"))]
    fn state_is_advertisement_building(adv_builder_state: &V1AdvertisementBuilderState) -> bool {
        matches!(adv_builder_state, V1AdvertisementBuilderState::Advertisement(_))
    }

    #[cfg(not(feature = "nocrypto"))]
    #[allow(clippy::expect_used)]
    #[test]
    fn test_build_section_fails_with_outstanding_section() {
        let mut adv_builder = V1AdvertisementBuilderInternals::new();

        let adv_builder_state =
            adv_builder.state.as_ref().expect("Adv builder state should be present.");
        assert!(state_is_advertisement_building(adv_builder_state));
        let section_index = adv_builder
            .encrypted_section_builder(empty_broadcast_cred(), V1VerificationMode::Mic)
            .expect("Should be able to build the first section.");
        assert_eq!(section_index, 0);

        assert!(adv_builder.state.is_some());
        let adv_builder_state =
            adv_builder.state.as_ref().expect("Adv builder state should be present.");
        assert!(!state_is_advertisement_building(adv_builder_state));

        let double_build_error = adv_builder
            .encrypted_section_builder(empty_broadcast_cred(), V1VerificationMode::Mic)
            .expect_err("Shouldn't be able to start a new section builder with an unclosed one.");
        assert_eq!(double_build_error, SectionBuilderError::UnclosedActiveSection);
    }

    #[cfg(not(feature = "nocrypto"))]
    fn empty_broadcast_cred() -> V1BroadcastCredential {
        V1BroadcastCredential::new([0; 32], [0; 16].into())
    }
}
