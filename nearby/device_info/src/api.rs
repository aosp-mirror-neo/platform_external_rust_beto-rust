// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
//
// you may not use this file except in compliance with the License.
//
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
//
// distributed under the License is distributed on an "AS IS" BASIS,
//
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
// See the License for the specific language governing permissions and
//
// limitations under the License.
//
//

use thiserror::Error;
use tokio::sync::watch::Receiver;

/// Device state.
pub trait DeviceState {
    /// Gets the current screen state.
    fn get_screen_state(&self) -> Result<ScreenState, SettingsError>;
    /// Starts watching the screen state.
    fn screen_state_watch(&self) -> Result<Receiver<ScreenState>, SettingsError>;
    /// Stops watching screen state and drops the event sender.
    fn stop_screen_state_watch(&self) -> Result<(), SettingsError>;
}

/// Device type.
#[derive(Debug)]
pub enum DeviceType {
    /// The current device is a phone.
    Phone,
    /// The current device is a laptop.
    Laptop,
    /// The current device is a tablet.
    Tablet,
    /// The current device is a desktop.
    Desktop,
    /// We were unable to determine the type of device.
    Unknown,
}

/// An error when interacting with device information settings.
#[derive(Error, Debug)]
pub enum SettingsError {
    /// Wrapper around OS-level errors, e.g. `windows::core::Error` for Windows.
    /// These typically mean something is very wrong with the system (e.g. OOM).
    #[error("OS error.")]
    System(Box<dyn std::error::Error + Send + Sync>),
    /// Reported when a bug occurs inside the library. Whenever a seemingly
    /// impossible error condition arises where you could call `expect()`,
    /// return this error instead.
    #[error("Internal error {0}")]
    Internal(String),
}

/// The current device screen state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenState {
    /// The screen is on.
    On,
    /// The screen is off.
    Off,
}
