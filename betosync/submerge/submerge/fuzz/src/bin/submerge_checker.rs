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

//! A simulation checker for the `submerge` implementation.

#![cfg_attr(fuzzing, no_main)]

use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};

use itertools::Itertools;
use log::debug;
use submerge_fuzz::{
    actor::{Actor, ActorContext, ActorMessage, ActorOp},
    log_msg, DeferredArbitrary, ExtraEntropy,
};

#[derive_fuzztest::fuzztest]
fn simulation(sim: SubmergeSimulation, arb: ExtraEntropy) {
    env_logger::builder().is_test(true).try_init();
    sim.test(arb)
}

#[derive(Debug, Default)]
pub struct NetworkHarness {
    messages: BTreeMap<usize, MessageEnvelope>,
    next_message_id: usize,
}

#[derive(Debug, Clone)]
pub struct MessageEnvelope {
    message: ActorMessage,
    message_id: usize,
    sender: u16,
    dest: u16,
}

impl ActorContext for NetworkHarness {
    fn send_message(&mut self, sender: u16, dest: u16, msg: ActorMessage) -> usize {
        let id = self.next_message_id;
        let old_value = self.messages.insert(
            id,
            MessageEnvelope {
                message: msg,
                message_id: id,
                sender,
                dest,
            },
        );
        assert!(old_value.is_none());
        self.next_message_id += 1;
        id
    }
}

const NUM_ACTORS: u16 = 4;

#[derive(Debug, Clone, Arbitrary)]
enum SimulatorOp {
    ActorOp {
        #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=(NUM_ACTORS - 1)))]
        node: u16,
        op: ActorOp,
    },
    DeliverMessage {
        #[arbitrary(with = |_: &mut Unstructured| Ok(DeferredArbitrary::Pending))]
        index: DeferredArbitrary<usize>,
        consume: bool,
    },
}

#[derive(Debug, Clone, Arbitrary)]
pub struct SubmergeSimulation {
    ops: Vec<SimulatorOp>,
}

impl SubmergeSimulation {
    pub fn test(self, mut arb: ExtraEntropy) {
        let mut unstructured = arb.unstructured();
        let mut network = NetworkHarness::default();
        let mut actors: [_; NUM_ACTORS as usize] =
            std::array::from_fn(|id| Actor::new(u16::try_from(id).unwrap(), 0..NUM_ACTORS));
        let mut document_actors = actors.map_mut(|actor| actor.document_actor());
        let ops_len = self.ops.len();
        let _ = (|| -> arbitrary::Result<_> {
            for op in self.ops {
                if unstructured.is_empty() {
                    break;
                }
                match op {
                    SimulatorOp::ActorOp { node, op } => {
                        document_actors[node as usize].apply_op(
                            op.clone(),
                            &mut network,
                            &mut unstructured,
                        )?;
                    }
                    SimulatorOp::DeliverMessage { mut index, consume } => {
                        let index = index.instantiate_with(|| {
                            unstructured.choose_index(network.next_message_id)
                        })?;
                        let msg_option = if consume {
                            network.messages.remove(index)
                        } else {
                            network.messages.get(index).cloned()
                        };
                        if let Some(msg_envelope) = msg_option {
                            debug!(
                                "Delivering message #{id} ({sender} -> {dest}). Consume={consume}",
                                id = msg_envelope.message_id,
                                sender = msg_envelope.sender,
                                dest = msg_envelope.dest
                            );
                            document_actors[msg_envelope.dest as usize].deliver_message(
                                msg_envelope.sender,
                                msg_envelope.message,
                                &mut network,
                            );
                        }
                    }
                }
            }
            Ok(())
        })();
        log_msg!(
            "==> Executing {} operations. {} messages generated",
            ops_len,
            network.next_message_id
        );

        while let Some((_, msg_envelope)) = network.messages.pop_last() {
            debug!(
                "Delivering remaining message #{id} ({sender} -> {dest})",
                id = msg_envelope.message_id,
                sender = msg_envelope.sender,
                dest = msg_envelope.dest
            );
            document_actors[msg_envelope.dest as usize].deliver_message(
                msg_envelope.sender,
                msg_envelope.message,
                &mut network,
            );
        }

        for (actor1, actor2) in document_actors.iter().tuple_windows() {
            // All nodes should have the same in-memory representation
            assert_eq!(
                actor1.doc_handle.snapshot(),
                actor2.doc_handle.snapshot(),
                "Snapshots for actors {} and {} differ",
                actor1.id,
                actor2.id
            );
        }

        for actor in actors.iter() {
            // All nodes should have the same persisted representation
            assert_eq!(
                actor.data_store.snapshot("test_doc"),
                actor.data_store.storage().persisted_doc,
                "Actor {}: in-memory snapshot should match persisted document",
                actor.id
            );
        }
    }
}

trait ArrayExt<T, const N: usize> {
    /// Similar to `array.map`, but operates on mutable references of the elements instead of
    /// consuming the original array.
    fn map_mut<'t, R>(&'t mut self, f: impl FnMut(&'t mut T) -> R) -> [R; N]
    where
        T: 't;
}

impl<T, const N: usize> ArrayExt<T, N> for [T; N] {
    fn map_mut<'t, R>(&'t mut self, f: impl FnMut(&'t mut T) -> R) -> [R; N]
    where
        T: 't,
    {
        let mut iter = self.iter_mut().map(f);
        std::array::from_fn(|_| iter.next().unwrap())
    }
}
