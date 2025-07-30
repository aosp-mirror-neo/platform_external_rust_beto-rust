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

use std::fmt::Debug;

use actor::{ActorClock, ActorContext, ActorMessage, ActorNetwork, ActorStorage};
use arbitrary::{Arbitrary, Unstructured};
use submerge::DataStoreConfig;

pub mod actor;
pub mod fuzz_metrics;

#[derive(Debug)]
pub struct TestDataStoreConfig;

impl DataStoreConfig for TestDataStoreConfig {
    type ValueType = Vec<u8>;
    type NodeId = u16;
    type Network = ActorNetwork;
    type Storage = ActorStorage;
    type TimestampProvider = ActorClock;
}

#[derive(Debug, Clone)]
pub struct MessageEnvelope {
    pub message: ActorMessage,
    pub message_id: usize,
    pub sender: u16,
    pub dest: u16,
}

/// A network harness simulating an eventual-delivery system.
///
/// This system may deliver messages out-of-order or deliver duplicate messages, but it guarantees
/// that each message is delivered at least once.
#[derive(Debug, Default)]
pub struct NetworkHarness {
    pub messages: Vec<MessageEnvelope>,
    pub next_message_id: usize,
}

impl ActorContext for NetworkHarness {
    /// Called by an actor when it wants to send a message.
    fn send_message(&mut self, sender: u16, dest: u16, msg: ActorMessage) -> usize {
        let id = self.next_message_id;
        self.messages.push(MessageEnvelope {
            message: msg,
            message_id: id,
            sender,
            dest,
        });
        self.next_message_id += 1;
        id
    }
}

#[derive(Debug, Clone)]
pub struct ExtraEntropy(Vec<u8>);

impl<'a> Arbitrary<'a> for ExtraEntropy {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Custom Arbitrary implementation that takes every byte that's remaining.
        Ok(ExtraEntropy(u.bytes(u.len())?.to_vec()))
    }
}

impl ExtraEntropy {
    pub fn unstructured(&mut self) -> Unstructured<'_> {
        Unstructured::new(&self.0)
    }
}

/// An arbitrary implementation that deferred its instantiation.
///
/// Instead of instantiating the whole test setup up-front, together with [`ExtraEntropy`] this
/// allows part of the test to be instantiated at runtime. This avoids wasting entropy (incoming
/// bytes from the fuzzer) on operations that won't be applicable in the current state.
#[derive(Debug, Clone)]
pub enum DeferredArbitrary<T> {
    Pending,
    Instantiated(T),
}

impl<T> DeferredArbitrary<T> {
    pub fn instantiate_with<'a>(
        &mut self,
        f: impl FnOnce() -> arbitrary::Result<T>,
    ) -> arbitrary::Result<&T>
    where
        T: Arbitrary<'a> + Debug,
    {
        *self = Self::Instantiated(f()?);
        match self {
            DeferredArbitrary::Pending => unreachable!(),
            DeferredArbitrary::Instantiated(value) => Ok(value),
        }
    }
}
