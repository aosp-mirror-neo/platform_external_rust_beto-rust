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

#![allow(dead_code, unused_imports)]
pub(crate) mod credential_manager;
pub(crate) mod credential_manager_impl;
/// Module for lazy discovery credential book.
pub mod lazy_book;
mod utils;

/// Represents metadata encrypted into a byte array.
pub type EncryptedMetadata = up_adv::credential::matched::MatchedCredentialWithId<Vec<u8>>;
