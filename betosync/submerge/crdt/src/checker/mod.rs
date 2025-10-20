// Copyright 2024 Google LLC
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

//! Module for testing and checking the invariants of a CRDT implementation.
//!
//! There are two main types of tests provided in this module:
//! * [`MergeInvariantTest`] tests the invariants required for state-based CRDTs directly on
//!   arbitrary instances of the given `CrdtState`.
//! * [`Simulation`][simulation::Simulation] tests the CRDT implementations by defining valid
//!   operations on a CRDT implementation, arbitrarily shuffling the resulting update messages, and
//!   check to ensure that the implementations converge.
//!
//! Note that both of these tests only test for invariants that apply to all CRDT implementations,
//! namely eventual consistency. Unit tests are still required to ensure that the resulting state
//! matches the user intent.
//!
//! _Requires the `checker` feature_.

use std::fmt::Debug;

use crate::CrdtState;
use arbitrary::{Arbitrary, Unstructured};

pub(crate) mod eventual_delivery;
pub mod simulation;
pub mod test_crdt_union;
pub mod test_fakes;
pub mod utils;

/// Test case for the invariant properties required for `merge` functions of state-based CRDTs.
///
/// This type implements `Arbitrary` and can be used in property-testing and fuzzing in order to
/// verify these invariants automatically (see the `derive-fuzztest` crate).
///
/// This test checks against collections of state that are valid, according to
/// [`CrdtState::is_valid_collection`], which by default is any instance allowed by the type system.
/// If there are additional conditions for convergence, (e.g. if your CRDT relies on globally unique
/// timestamps), you should implement `is_valid_collection`. The helper functions
/// [`is_uniq`][crate::checker::utils::IterExt::is_uniq] and
/// [`iter_pairs`][crate::checker::utils::IterExt::iter_pairs_unordered] may be useful.
///
/// # Example
///
/// This example implements a last-writer-wins register that requires globally unique timestamps.
///
/// ```
/// use crdt::{checker::MergeInvariantTest, CrdtState, MergeError};
///
/// #[derive(Debug, Clone, PartialEq, Eq)]
/// pub struct LwwValue { timestamp: u64, value: String }
///
/// impl CrdtState for LwwValue {
///     fn merge(a: &Self, b: &Self) -> Result<Self, MergeError> {
///         Ok(std::cmp::max_by_key(a, b, |v| v.timestamp).clone())
///     }
///
///     #[cfg(any(test, feature = "checker"))]
///     fn is_valid_collection<'a>(collection: impl IntoIterator<Item = &'a Self>) -> bool
///     where
///         Self: 'a,
///     {
///         use crdt::checker::utils::IterExt as _;
///
///         collection.into_iter().map(|s| &s.timestamp).is_uniq()
///     }
/// }
///
/// #[derive_fuzztest::fuzztest]
/// fn invariant_test(test: MergeInvariantTest<LwwValue>) {
///     test.test()
/// }
/// ```
#[derive(Clone, Debug)]
pub enum MergeInvariantTest<S: CrdtState + Eq + Debug> {
    /// Test associativity `merge(merge(a, b), c) == merge(a, merge(b, c))`
    Associativity(S, S, S),
    /// Test commutativity `merge(a, b) == merge(b, a)`
    Commutativity(S, S),
    /// Test idempotence `merge(a, a) == a`
    Idempotence(S),
}

impl<S: CrdtState + Eq + Debug> MergeInvariantTest<S> {
    /// Run the test, panicking if the invariant is violated.
    pub fn test(self) {
        match self {
            Self::Associativity(s1, s2, s3) => {
                assert_eq!(
                    S::merge(&S::merge(&s1, &s2).expect("merge must not fail"), &s3)
                        .expect("merge must not fail"),
                    S::merge(&s1, &S::merge(&s2, &s3).expect("merge must not fail"))
                        .expect("merge must not fail"),
                    "Associativity violated:\ns1={s1:#?}\ns2={s2:#?}\ns3={s3:#?}",
                )
            }
            Self::Commutativity(s1, s2) => {
                assert_eq!(
                    S::merge(&s1, &s2).expect("merge must not fail"),
                    S::merge(&s2, &s1).expect("merge must not fail"),
                    "Commutativity violated:\ns1={s1:#?}\ns2={s2:#?}"
                )
            }
            Self::Idempotence(s1) => {
                assert_eq!(
                    S::merge(&s1, &s1).expect("merge must not fail"),
                    s1,
                    "Idempotence violated:\ns1={s1:#?}"
                )
            }
        }
    }
}

impl<'a, S> Arbitrary<'a> for MergeInvariantTest<S>
where
    S: CrdtState + Eq + Debug + Arbitrary<'a>,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=2)? {
            0 => {
                let s1 = S::arbitrary(u)?;
                let s2 = S::arbitrary(u)?;
                let s3 = S::arbitrary(u)?;
                if S::is_valid_collection([&s1, &s2, &s3]) {
                    Ok(MergeInvariantTest::Associativity(s1, s2, s3))
                } else {
                    Err(arbitrary::Error::IncorrectFormat)
                }
            }
            1 => {
                let s1 = S::arbitrary(u)?;
                let s2 = S::arbitrary(u)?;
                if S::is_valid_collection([&s1, &s2]) {
                    Ok(MergeInvariantTest::Commutativity(s1, s2))
                } else {
                    Err(arbitrary::Error::IncorrectFormat)
                }
            }
            2 => {
                let s = S::arbitrary(u)?;
                if S::is_valid_collection([&s]) {
                    Ok(MergeInvariantTest::Idempotence(s))
                } else {
                    Err(arbitrary::Error::IncorrectFormat)
                }
            }
            _ => Err(arbitrary::Error::IncorrectFormat),
        }
    }
}

#[cfg(test)]
pub mod tests;
