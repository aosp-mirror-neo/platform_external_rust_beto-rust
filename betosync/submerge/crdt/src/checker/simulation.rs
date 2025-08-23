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

//! Tests for CRDT convergence by simulating the operations, shuffling the resulting update messages
//! and verifying that the merge results are consistent.
//!
//! See [`Simulation`].

use crate::{
    delta::{AsDeltaMut, DeltaMut, DeltaOwned},
    CrdtState,
};
use arbitrary::Arbitrary;
use derive_where::derive_where;
use distributed_time::fake_timestamp::FakeTimestamp;
use std::{collections::BTreeMap, fmt::Debug};

use super::{
    eventual_delivery::EventualDeliveryScenario,
    test_fakes::{FakeContext, TriState},
};

/// An operation that can be tested in a [`Simulation`].
///
/// Typically an operation is an enum which represents all possible operations for the state `S`
/// under test. These operations may include environmental changes like clock value updates. An
/// operation should implement [`Arbitrary`] so that an arbitrary [`Simulation`] can be created
/// during fuzzing or property testing.
pub trait Operation<S, N: Ord + Eq> {
    /// Applies this operation to the given `state`.
    ///
    /// The operation should mutate the given `state`. If `state` contains a non-empty delta
    /// component when this call returns, the simulation will record the delta as the update
    /// message.
    ///
    /// Implementations can also add `assert!` or `panic!` to check for invariants that should be
    /// kept after an operation is applied.
    ///
    /// # Params
    /// * `state` - the current state that the operation should be applied to.
    /// * `context` - the context information for this execution.
    fn apply(self, state: DeltaMut<S>, context: &SimulationContext<N>);
}

/// A simulation test for a CRDT implementation `S`.
///
/// The simulation starts at an arbitrary `init_state: S`, applies an arbitrary list of operations
/// `OP` and gathers the update messages generated during [`Operation::apply`]. It then shuffles the
/// gathered messages according to eventual delivery rules (i.e. messages can be reordered and
/// duplicated, but not lost). The merge result from the shuffled update messages is then compared
/// against the merge result from unshuffled ones, and makes sure that they are equal (i.e.
/// satisfies eventual consistency).
///
/// ## Params
/// * `S`: The CRDT state to test against.
/// * `OP`: The operations to be applied onto `S`, typically an enum.
/// * `N`: The Node ID. The default is [`TriState`], which means the system has at most 3 nodes.
///
/// # Example
/// ```
/// use arbitrary::Arbitrary;
/// use crdt::{
///     checker::{
///         simulation::{Operation, Simulation, SimulationContext},
///         test_fakes::TriState,
///     },
///     delta::{AsDeltaMut, AsDeltaRef, DeltaMut},
///     CrdtState,
/// };
///
/// /// Our trivial example CRDT.
/// #[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Arbitrary)]
/// pub struct IntegerMaxCrdt(u8);
///
/// /// Merges by taking the max of the two values.
/// impl CrdtState for IntegerMaxCrdt {
///     fn merge(a: &Self, b: &Self) -> Self {
///         a.max(b).clone()
///     }
/// }
///
/// /// Define the operations that can be performed on the CRDT.
/// #[derive(Debug, Clone, Arbitrary)]
/// pub enum IntegerMaxOp {
///     /// Only operation supported in this simulation is setting the integer value
///     SetInteger { value: u8 },
///     // Operations outside of the CRDT, like setting the wall clock can also be defined here.
/// }
///
/// impl Operation<IntegerMaxCrdt, TriState> for IntegerMaxOp {
///     fn apply(self, mut state: DeltaMut<IntegerMaxCrdt>, _ctx: &SimulationContext<TriState>) {
///         match self {
///             IntegerMaxOp::SetInteger { value } => {
///                 if value > state.as_ref().merge().0 {
///                     state.delta_mut().0 = value;
///                 }
///             }
///         }
///     }
/// }
///
/// #[derive_fuzztest::fuzztest]
/// fn simulator(sim: Simulation<IntegerMaxCrdt, IntegerMaxOp>) {
///     let _ = sim.run();
/// }
/// ```
#[derive(Debug, Clone, Arbitrary)]
pub struct Simulation<S, OP, N = TriState>
where
    S: CrdtState,
    OP: Operation<S, N> + Debug,
    N: Ord,
{
    init_state: S,
    operations: Vec<SimulationOperation<N, OP>>,
    delivery_scenario: EventualDeliveryScenario,
}

impl<S, OP, N> Simulation<S, OP, N>
where
    S: CrdtState + Clone + Eq + Debug + Default,
    OP: Operation<S, N> + Debug + Clone,
    N: Ord,
{
    /// Drains the operations from `self.operations` and gathers the list of resulting messages.
    fn sent_messages(&mut self) -> (S, Vec<S>) {
        let mut context = SimulationContext::<N>::default();
        let mut update_messages = Vec::with_capacity(self.operations.len());
        update_messages.push(self.init_state.clone());
        let mut current_state = self.init_state.clone();
        for op in self.operations.drain(..) {
            match op {
                SimulationOperation::RegularOp(op) => {
                    let mut delta = DeltaOwned::new(&current_state);
                    op.apply(delta.as_mut(), &context);
                    let delta_component = delta.into_delta();
                    current_state = CrdtState::merge(&current_state, &delta_component);
                    update_messages.push(delta_component);
                }
                SimulationOperation::ContextOp(op) => {
                    op.apply(&mut context);
                }
            }
        }
        (current_state, update_messages)
    }

    /// Run the simulation test and assert that the results converge after arbitrarily shuffling
    /// the update messages.
    ///
    /// # Returns
    /// Returns the two resulting states (which are equal according to `Eq`), or `None` if a valid
    /// shuffling cannot be generated (and the test should be discarded). The returned states can be
    /// used to verify additional invariants specific to the CRDT implementation, for example if
    /// there are states that should not be reachable.
    ///
    /// # Panics
    /// Panics if the resulting states from the shuffled and unshuffled operations are not equal.
    pub fn run(mut self) -> Option<(S, S)> {
        let (final_state, sent_messages) = self.sent_messages();
        let delivered_messages = self.delivery_scenario.scramble(&sent_messages)?;
        let state1 = sent_messages
            .into_iter()
            .fold(self.init_state.clone(), |a, b| CrdtState::merge(&a, &b));
        let state2 = delivered_messages
            .into_iter()
            .fold(self.init_state.clone(), |a, b| CrdtState::merge(&a, &b));

        assert_eq!(state1, state2);
        assert_eq!(state1, final_state);
        Some((state1, state2))
    }
}

#[derive(Clone, Debug, Arbitrary)]
enum SimulationOperation<N: Ord, OP> {
    RegularOp(OP),
    ContextOp(SimulationContextOperation<N>),
}

/// Stores contextual information for all the nodes in a simulation.
///
/// Currently this only stores the wall-clock timestamps for each node.
#[derive(Debug, Clone, Arbitrary)]
#[derive_where(Default)]
pub struct SimulationContext<N: Ord + Eq> {
    timestamps: BTreeMap<N, FakeTimestamp>,
}

impl<N> SimulationContext<N>
where
    N: Ord + Eq + Clone,
{
    /// Get the [`UpdateContext`][crate::UpdateContext] for the given node in this simulation.
    pub fn context(&self, node: &N) -> FakeContext<N, u8> {
        FakeContext::new(
            node.clone(),
            self.timestamps.get(node).cloned().unwrap_or_default().0,
        )
    }
}

/// A mutation operation on the [`SimulationContext`].
#[derive(Debug, Clone, Arbitrary)]
#[allow(missing_docs)]
enum SimulationContextOperation<N: Ord + Eq> {
    Increment { node: N },
    Decrement { node: N },
    SetValue { node: N, timestamp: FakeTimestamp },
}

impl<N: Ord + Eq> SimulationContextOperation<N> {
    /// Applies the operation on the given simulation context.
    fn apply(self, state: &mut SimulationContext<N>) {
        match self {
            SimulationContextOperation::Increment { node } => {
                let timestamp = state.timestamps.entry(node).or_default();
                if let Some(incremented) = timestamp.0.checked_add(1) {
                    *timestamp = FakeTimestamp(incremented);
                }
            }
            SimulationContextOperation::Decrement { node } => {
                let timestamp = state.timestamps.entry(node).or_default();
                if let Some(decremented) = timestamp.0.checked_sub(1) {
                    *timestamp = FakeTimestamp(decremented);
                }
            }
            SimulationContextOperation::SetValue { node, timestamp } => {
                let _ = state.timestamps.insert(node, timestamp);
            }
        }
    }
}
