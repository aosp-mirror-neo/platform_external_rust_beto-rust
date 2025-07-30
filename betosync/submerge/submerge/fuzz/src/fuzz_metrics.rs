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

use std::cell::Cell;

thread_local! {
    pub static COUNTER: Cell<usize> = Cell::default();
}

/// Log a message during fuzzing. This will be executed one every 8192 fuzz runs so as to not spam
/// the terminal output.
#[macro_export]
macro_rules! log_msg {
    ($($arg:tt)*) => {
        if $crate::fuzz_metrics::COUNTER.replace($crate::fuzz_metrics::COUNTER.get().wrapping_add(1)) % 8192 == 0 {
            eprintln!($($arg)*);
        }
    };
}
