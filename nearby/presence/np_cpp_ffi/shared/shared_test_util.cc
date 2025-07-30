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

#include "shared_test_util.h"

#include <cstdlib>

#include "nearby_protocol.h"

using nearby_protocol::KEY_SEED_SIZE;
using nearby_protocol::HMAC_TAG_SIZE;

std::string PanicReasonToString(nearby_protocol::PanicReason reason) {
  switch (reason) {
    case nearby_protocol::PanicReason::EnumCastFailed: {
      return "EnumCastFailed";
    }
    case nearby_protocol::PanicReason::AssertFailed: {
      return "AssertFailed";
    }
    case nearby_protocol::PanicReason::InvalidStackDataStructure: {
      return "InvalidStackDataStructure";
    }
    case np_ffi::internal::PanicReason::ExceededMaxHandleAllocations:
      return "ExceededMaxHandleAllocations";
  }
}

void test_panic_handler(nearby_protocol::PanicReason reason) {
  std::cout << "Panicking! Reason: " << PanicReasonToString(reason);
  std::abort();
}

std::string generate_hex_string(const size_t length) {
  std::string result;
  result.reserve(length);

  // hexadecimal characters
  char hex_characters[] = {'0', '1', '2', '3', '4', '5', '6', '7',
                           '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'};

  for (size_t i = 0; i < length; i++) {
    result.push_back(hex_characters[rand() % 16]);  // NOLINT(cert-msc50-cpp)
  }

  return result;
}

nearby_protocol::V0MatchableCredential GenerateRandomCredentialV0() {
  auto key_seed = create_random_array<32>();
  auto legacy_metadata_key_hmac = create_random_array<32>();
  auto encrypted_metadata_bytes = create_random_array<200>();
  nearby_protocol::MatchedCredentialData matched_cred(rand(),
                                                      encrypted_metadata_bytes);
  return {key_seed, legacy_metadata_key_hmac, matched_cred};
}

nearby_protocol::V1MatchableCredential GenerateRandomCredentialV1() {
  auto key_seed = create_random_array<KEY_SEED_SIZE>();
  auto expected_mic_short_salt_identity_token_hmac = create_random_array<HMAC_TAG_SIZE>();
  auto expected_mic_extended_salt_identity_token_hmac = create_random_array<HMAC_TAG_SIZE>();
  auto encrypted_metadata_bytes = create_random_array<200>();
  nearby_protocol::MatchedCredentialData matched_cred(rand(),
                                                      encrypted_metadata_bytes);
  return nearby_protocol::V1MatchableCredential(
      key_seed,
      expected_mic_short_salt_identity_token_hmac,
      expected_mic_extended_salt_identity_token_hmac,
      matched_cred);
}
