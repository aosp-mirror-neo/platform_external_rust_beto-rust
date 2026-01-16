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

use crate::Seconds;
use coset::cbor::Value;
use coset::cwt;

pub(crate) fn get_private_claim_value(
    claims_rest_map: &[(cwt::ClaimName, Value)],
    claim_id: i64,
) -> Option<&Value> {
    let cwt_claim_name = cwt::ClaimName::PrivateUse(claim_id);
    claims_rest_map.iter().find(|(k, _)| *k == cwt_claim_name).map(|(_, v)| v)
}

pub(crate) fn get_timestamp_claim_as_seconds(
    timestamp: &Option<cwt::Timestamp>,
) -> Option<Seconds> {
    timestamp.as_ref().and_then(|ts| match ts {
        cwt::Timestamp::WholeSeconds(secs) => (*secs).try_into().ok().map(Seconds),
        _ => None,
    })
}

#[cfg(feature = "std")]
pub(crate) fn get_current_system_time() -> Seconds {
    Seconds(
        u32::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the epoch will always be earlier than now")
                .as_secs(),
        )
        .expect(
            "A u32 will suffice for representing seconds from the unix epoch until\
         November 27 of 2140. If this code is still running in that year it will crash here,\
         but that's a problem I'll leave for the future developers to deal with ;)",
        ),
    )
}

#[cfg(not(feature = "std"))]
pub(crate) fn get_current_system_time() -> Seconds {
    todo!("provide hooks for no_std environments to obtain system time via their own mechanism")
}
