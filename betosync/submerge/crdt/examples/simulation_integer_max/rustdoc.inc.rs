use arbitrary::Arbitrary;
use crdt::{
    checker::{
        simulation::{Operation, Simulation, SimulationContext},
        test_fakes::TriState,
    },
    delta::{AsDeltaMut, AsDeltaRef, DeltaMut},
    CrdtState,
};

/// Our trivial example CRDT.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Arbitrary)]
pub struct IntegerMaxCrdt(u8);

/// Merges by taking the max of the two values.
impl CrdtState for IntegerMaxCrdt {
    fn merge(a: &Self, b: &Self) -> Self {
        a.max(b).clone()
    }
}

/// Define the operations that can be performed on the CRDT.
#[derive(Debug, Clone, Arbitrary)]
pub enum IntegerMaxOp {
    /// Only operation supported in this simulation is setting the integer value
    SetInteger { value: u8 },
    // Operations outside of the CRDT, like setting the wall clock can also be defined here.
}

impl Operation<IntegerMaxCrdt, TriState> for IntegerMaxOp {
    fn apply(self, mut state: DeltaMut<IntegerMaxCrdt>, _ctx: &SimulationContext<TriState>) {
        match self {
            IntegerMaxOp::SetInteger { value } => {
                if value > state.as_ref().merge().0 {
                    state.delta_mut().0 = value;
                }
            }
        }
    }
}

#[derive_fuzztest::fuzztest]
fn simulator(sim: Simulation<IntegerMaxCrdt, IntegerMaxOp>) {
    let _ = sim.run();
}
