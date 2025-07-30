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

//! Test implementation of [`CrdtUnion`].
//!
//! See [`TestCrdtUnion`].

use std::{convert::Infallible, fmt::Debug};

use crate::{
    delta::{AsDeltaMut, AsDeltaRef, DeltaMut, DeltaRef},
    lww_crdt_container::{CrdtUnion, IncompatibleType, TryAsChild},
    ApplyChanges, CrdtState, HasPlainRepresentation, ToPlain, UpdateContext,
};
use arbitrary::Arbitrary;
use distributed_time::vector_clock::VectorClock;

/// A test implementation of [`CrdtUnion`] with two variants, which merges by simply taking the max
/// if values are of the same enum variant, or fails if values are of different enum variants.
#[derive(Debug, Clone, Arbitrary, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum TestCrdtUnion {
    Type1(TestVariant1),
    Type2(TestVariant2),
}

impl From<TestVariant1> for TestCrdtUnion {
    fn from(value: TestVariant1) -> Self {
        Self::Type1(value)
    }
}

impl From<TestVariant2> for TestCrdtUnion {
    fn from(value: TestVariant2) -> Self {
        Self::Type2(value)
    }
}

/// A enum analogous to [`TestCrdtUnion`], but contains [`DeltaRef`] of each variant.
#[derive(Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum TestCrdtUnionRef<'d> {
    Type1(DeltaRef<'d, TestVariant1>),
    Type2(DeltaRef<'d, TestVariant2>),
}

impl TestCrdtUnionRef<'_> {
    /// Combines the base part and the delta part and return the resulting merged component.
    ///
    /// See [`CrdtState::merge`] for details on the merge operation.
    pub fn merge(&self) -> TestCrdtUnion {
        match self {
            Self::Type1(r) => TestCrdtUnion::Type1(r.merge()),
            Self::Type2(r) => TestCrdtUnion::Type2(r.merge()),
        }
    }
}

/// A enum analogous to [`TestCrdtUnion`], but contains [`DeltaMut`] of each variant.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum TestCrdtUnionMut<'d> {
    Type1(DeltaMut<'d, TestVariant1>),
    Type2(DeltaMut<'d, TestVariant2>),
}

impl TestCrdtUnionMut<'_> {
    /// Combines the base part and the delta part and return the resulting merged component.
    ///
    /// See [`CrdtState::merge`] for details on the merge operation.
    pub fn merge(&self) -> TestCrdtUnion {
        match self {
            Self::Type1(r) => TestCrdtUnion::Type1(r.as_ref().merge()),
            Self::Type2(r) => TestCrdtUnion::Type2(r.as_ref().merge()),
        }
    }
}

/// Test variant used in [`TestCrdtUnion`].
#[derive(Default, Debug, Clone, Copy, Arbitrary, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestVariant1(pub u8);

impl CrdtState for TestVariant1 {
    fn merge(a: &Self, b: &Self) -> Self {
        Self(a.0.max(b.0))
    }
}

impl DeltaMut<'_, TestVariant1> {
    /// Sets the value of this.
    pub fn set_value(&mut self, value: u8) {
        self.delta.0 = value
    }
}

impl DeltaRef<'_, TestVariant1> {
    /// Gets the value.
    pub fn get_value(&self) -> u8 {
        self.delta.or(self.base).map(|v| v.0).unwrap_or_default()
    }
}

/// Test variant used in [`TestCrdtUnion`].
#[derive(Default, Debug, Clone, Copy, Arbitrary, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestVariant2(pub u8);

impl CrdtState for TestVariant2 {
    fn merge(a: &Self, b: &Self) -> Self {
        Self(a.0.max(b.0))
    }
}

impl DeltaMut<'_, TestVariant2> {
    /// Sets the value of this.
    pub fn set_value(&mut self, value: u8) {
        self.delta.0 = value
    }
}

impl DeltaRef<'_, TestVariant2> {
    /// Gets the value.
    pub fn get_value(&self) -> u8 {
        self.delta.or(self.base).map(|v| v.0).unwrap_or_default()
    }
}

impl<N: Ord> TryAsChild<TestVariant1, N> for TestCrdtUnion {
    fn try_as_child_ref(this: Self::DeltaRef<'_>) -> Option<DeltaRef<'_, TestVariant1>> {
        match this {
            TestCrdtUnionRef::Type1(v) => Some(v),
            TestCrdtUnionRef::Type2(_) => None,
        }
    }

    fn try_as_child_mut(this: Self::DeltaMut<'_>) -> Option<DeltaMut<'_, TestVariant1>> {
        match this {
            TestCrdtUnionMut::Type1(v) => Some(v),
            TestCrdtUnionMut::Type2(_) => None,
        }
    }
}

impl<N: Ord> TryAsChild<TestVariant2, N> for TestCrdtUnion {
    fn try_as_child_ref(this: Self::DeltaRef<'_>) -> Option<DeltaRef<'_, TestVariant2>> {
        match this {
            TestCrdtUnionRef::Type1(_) => None,
            TestCrdtUnionRef::Type2(v) => Some(v),
        }
    }

    fn try_as_child_mut(this: Self::DeltaMut<'_>) -> Option<DeltaMut<'_, TestVariant2>> {
        match this {
            TestCrdtUnionMut::Type1(_) => None,
            TestCrdtUnionMut::Type2(v) => Some(v),
        }
    }
}

impl<N: Ord> CrdtUnion<N> for TestCrdtUnion {
    type DeltaRef<'d> = TestCrdtUnionRef<'d>;
    type DeltaMut<'d> = TestCrdtUnionMut<'d>;

    fn try_merge(a: &Self, b: &Self) -> Result<Self, IncompatibleType> {
        match (a, b) {
            (TestCrdtUnion::Type1(state_a), TestCrdtUnion::Type1(state_b)) => {
                Ok(TestCrdtUnion::Type1(*state_a.max(state_b)))
            }
            (TestCrdtUnion::Type2(state_a), TestCrdtUnion::Type2(state_b)) => {
                Ok(TestCrdtUnion::Type2(*state_a.max(state_b)))
            }
            _ => Err(IncompatibleType),
        }
    }

    #[allow(clippy::panic)]
    fn create_ref<'d>(
        base: Option<&'d Self>,
        delta: Option<&'d Self>,
    ) -> Option<Self::DeltaRef<'d>> {
        match (base, delta) {
            (None, None) => None,
            (None, Some(Self::Type1(t1))) => Some(TestCrdtUnionRef::Type1(DeltaRef {
                base: None,
                delta: Some(t1),
            })),
            (None, Some(Self::Type2(t2))) => Some(TestCrdtUnionRef::Type2(DeltaRef {
                base: None,
                delta: Some(t2),
            })),
            (Some(Self::Type1(t1)), None) => Some(TestCrdtUnionRef::Type1(DeltaRef {
                base: Some(t1),
                delta: None,
            })),
            (Some(Self::Type2(t2)), None) => Some(TestCrdtUnionRef::Type2(DeltaRef {
                base: Some(t2),
                delta: None,
            })),
            (Some(Self::Type1(base)), Some(Self::Type1(delta))) => {
                Some(TestCrdtUnionRef::Type1(DeltaRef {
                    base: Some(base),
                    delta: Some(delta),
                }))
            }
            (Some(Self::Type2(base)), Some(Self::Type2(delta))) => {
                Some(TestCrdtUnionRef::Type2(DeltaRef {
                    base: Some(base),
                    delta: Some(delta),
                }))
            }
            _ => panic!("Incompatible type"),
        }
    }

    #[allow(clippy::panic)]
    fn create_mut<'d>(base: Option<&'d Self>, delta: &'d mut Self) -> Option<Self::DeltaMut<'d>> {
        match (base, delta) {
            (None, Self::Type1(t1)) => Some(TestCrdtUnionMut::Type1(DeltaMut {
                base: None,
                delta: t1,
            })),
            (None, Self::Type2(t2)) => Some(TestCrdtUnionMut::Type2(DeltaMut {
                base: None,
                delta: t2,
            })),
            (Some(Self::Type1(base)), Self::Type1(delta)) => {
                Some(TestCrdtUnionMut::Type1(DeltaMut {
                    base: Some(base),
                    delta,
                }))
            }
            (Some(Self::Type2(base)), Self::Type2(delta)) => {
                Some(TestCrdtUnionMut::Type2(DeltaMut {
                    base: Some(base),
                    delta,
                }))
            }
            _ => panic!("Incompatible type"),
        }
    }

    fn create_matching_default(example: &Self) -> Self {
        match example {
            TestCrdtUnion::Type1(_) => TestCrdtUnion::Type1(Default::default()),
            TestCrdtUnion::Type2(_) => TestCrdtUnion::Type2(Default::default()),
        }
    }

    fn calculate_delta(&self, _base_version: &VectorClock<N>) -> Self {
        self.clone()
    }
}

impl<N: Ord> HasPlainRepresentation<N> for TestCrdtUnion {
    type Plain = TestCrdtUnion;
    type Error = Infallible;

    fn init_from_plain(
        _ctx: &impl UpdateContext<N>,
        plain: Self::Plain,
    ) -> Result<Self, Self::Error> {
        Ok(plain)
    }
}

impl ToPlain for TestCrdtUnionRef<'_> {
    type Plain = TestCrdtUnion;

    fn to_plain(&self) -> TestCrdtUnion {
        self.merge()
    }
}

impl<N> ApplyChanges<N> for TestCrdtUnionMut<'_> {
    type Plain = TestCrdtUnion;

    type Error = Infallible;

    fn apply_changes(
        &mut self,
        _ctx: &impl UpdateContext<N>,
        plain: Self::Plain,
    ) -> Result<bool, Self::Error> {
        match (self, plain) {
            (TestCrdtUnionMut::Type1(r1), TestCrdtUnion::Type1(v1)) => *r1.delta_mut() = v1,
            (TestCrdtUnionMut::Type2(r2), TestCrdtUnion::Type2(v2)) => *r2.delta_mut() = v2,
            _ => unreachable!(),
        }
        Ok(false)
    }
}
