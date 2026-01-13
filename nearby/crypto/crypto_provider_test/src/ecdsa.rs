// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate alloc;
extern crate std;

use crypto_provider::ecdsa::{P256EcdsaProvider, PublicKey, RawP256Signature, Signature};
pub use wycheproof::ecdsa::TestName as EcdsaTestName;
use wycheproof::TestResult;

/// Runs set of ECDSA wycheproof test vectors against a provided ecdsa implementation
/// Tests vectors from Project Wycheproof: <https://github.com/google/wycheproof>.
/// # Arguments
/// - test_name - The test name to load vectors from. Should correspond to the `EcdsaCurve`.
pub fn run_wycheproof_test_vectors<E>(test_name: wycheproof::ecdsa::TestName)
where
    E: P256EcdsaProvider,
{
    let test_set =
        wycheproof::ecdsa::TestSet::load(test_name).expect("should be able to load test set");

    for test_group in test_set.test_groups {
        let public_key = test_group.key.key;
        for test in test_group.tests {
            let tc_id = test.tc_id;
            let comment = test.comment;
            let sig = test.sig;
            let msg = test.msg;
            let valid = match test.result {
                TestResult::Invalid => false,
                TestResult::Valid | TestResult::Acceptable => true,
            };
            let result =
                run_wycheproof_test::<E>(public_key.as_slice(), sig.as_slice(), msg.as_slice());
            if valid {
                if let Err(desc) = result {
                    panic!(
                        "\n\
                         Failed test {tc_id}: {desc}\n\
                         msg:\t{msg:?}\n\
                         sig:\t{sig:?}\n\
                         comment:\t{comment:?}\n",
                    );
                }
            } else {
                assert!(result.is_err())
            }
        }
    }
}

fn run_wycheproof_test<E>(pub_key: &[u8], sig: &[u8], msg: &[u8]) -> Result<(), &'static str>
where
    E: P256EcdsaProvider,
{
    let pub_key = E::P256PublicKey::from_bytes(pub_key.try_into().unwrap())
        .map_err(|_| "Invalid public key bytes")?;

    let raw_sig: RawP256Signature = sig.try_into().map_err(|_| "Invalid length signature")?;
    let signature = E::P256Signature::from_bytes(&raw_sig).map_err(|_| "Invalid signature")?;

    pub_key.verify_strict(msg, &signature).map_err(|_| "Signature verification failed")
}
