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

use arbitrary::Arbitrary;

/// Simulation of an eventual delivery system that may arbitrarily reorder or duplicate the given
/// messages
#[derive(Debug, Clone, Arbitrary)]
pub struct EventualDeliveryScenario {
    pub message_scrambling_guide: Vec<usize>,
}

impl EventualDeliveryScenario {
    /// Returns the scrambled list of input, or `None` if the scrambling guide contained in this
    /// struct does not have enough information to satisfy eventual delivery. The returned list, if
    /// returned, is guaranteed to contain each message given in `sent_messages` at least once, to
    /// simulate eventual delivery.
    ///
    /// This consumes the eventual delivery scenario so that the input data is not reused for
    /// scrambling other messages. If needed, the caller can create multiple delivery scenarios for
    /// different packets that needs to be scrambled independently.
    pub fn scramble<T: Clone>(mut self, sent_messages: &[T]) -> Option<Vec<T>> {
        if sent_messages.is_empty() {
            return Some(vec![]);
        }
        let mut output = sent_messages.to_vec();
        let len = sent_messages.len();
        // Shuffle with Fisher-Yates algorithm
        for i in 1..len {
            let rand_index = sample_from(&mut self.message_scrambling_guide, len - i)?;
            output.swap(len - i, rand_index)
        }

        // For the remaining elements, copy them as duplicate messages
        while let (Some(copy_from), Some(new_index)) = (
            sample_from(&mut self.message_scrambling_guide, sent_messages.len()),
            sample_from(&mut self.message_scrambling_guide, output.len()),
        ) {
            output.insert(new_index, sent_messages[copy_from].clone());
        }

        Some(output)
    }
}

/// Given an `input` that is uniformly random between `[0, usize::MAX]`, return an input between
/// `[0, max)` that is not biased. Returns `None` if it is not possible to clamp without introducing
/// bias.
fn clamp_uniform_sample(input: usize, max: usize) -> Option<usize> {
    let max_admissible = usize::MAX - (usize::MAX % max);
    if input <= max_admissible {
        Some(input % max)
    } else {
        None
    }
}

/// Sample from the given source (pop items from the end of the vec), and clamping them within the
/// range `[0, max)`. It will retry until the source runs out of elements, or until a maximum of 256
/// retries is reached.
fn sample_from(source: &mut Vec<usize>, max: usize) -> Option<usize> {
    for _ in 0..256 {
        if let Some(output) = clamp_uniform_sample(source.pop()?, max) {
            return Some(output);
        }
    }
    panic!("Failed to sample from source without bias")
}

#[cfg(test)]
mod tests {
    use super::EventualDeliveryScenario;

    #[derive_fuzztest::proptest]
    fn scramble_len(input: Vec<u8>, scenario: EventualDeliveryScenario) {
        if let Some(scrambled) = scenario.scramble(&input) {
            assert!(scrambled.len() >= input.len());
            for i in input {
                assert!(scrambled.contains(&i));
            }
        }
    }
}
