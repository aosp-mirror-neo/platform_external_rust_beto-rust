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

use crate::api::{DeviceState, DeviceType, SettingsError};
use cfg_if::cfg_if;

pub(crate) fn get() -> Result<Box<dyn DeviceState>, SettingsError> {
    todo!()
}

// We want to panic to make sure any users of this function implement it for their respective platform.
#[allow(clippy::panic)]
#[cfg_attr(any(target_os = "windows", target_vendor = "apple"), allow(unsafe_code))]
pub(crate) fn device_name() -> String {
    cfg_if! {
        if #[cfg(target_os = "ios")] {
            use objc2_ui_kit::UIDevice;

            dispatch2::run_on_main(|mtm| {
                let ui_device = UIDevice::currentDevice(mtm);
                // SAFETY: This is a generated function, and is safe to call.
                let name = unsafe { ui_device.name() };
                // SAFETY: UTF8String is a generated function, and is safe to call. The underlying
                // string is guaranteed to be valid UTF-8 by the documentation from Apple.
                let slice = unsafe { std::slice::from_raw_parts(name.UTF8String() as *const u8, name.len()) };
                String::from_utf8(slice.to_vec()).ok().unwrap_or_default()
            })
        } else if #[cfg(target_os = "macos")] {
            #[allow(deprecated)]
            let name =  objc2_foundation::NSHost::currentHost().localizedName();
            // SAFETY: UTF8String is a generated function, and is safe to call. The underlying
            // string is guaranteed to be valid UTF-8 by the documentation from Apple.
            let bytes = name.map(|ns_str| unsafe { std::slice::from_raw_parts(ns_str.UTF8String() as *const u8, ns_str.len()) }.to_vec());

            // This string should always be valid UTF-8 according to the documentation from Apple.
            #[allow(clippy::expect_used)]
            bytes.map(|bytes| String::from_utf8(bytes.to_vec()).expect("Should be valid UTF-8!")).unwrap_or_default()
        } else if #[cfg(target_os = "windows")] {
            use windows::core::PWSTR;
            use windows::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};

            // Get length of the device name.
            let mut size = 0u32;
            // SAFETY: GetComputerNameExW is a generated function, and is safe to call.
            let _ = unsafe { GetComputerNameExW(ComputerNameDnsHostname, None, &mut size) };
            // Get the device name.
            let mut buf = vec![0u16; size as usize];
            // SAFETY: GetComputerNameExW is a generated function, and is safe to call.
            let _ = unsafe { GetComputerNameExW(ComputerNameDnsHostname, Some(PWSTR(buf.as_mut_ptr())), &mut size) };
            // Since this is a wide string, we must use UTF-16 here.
            String::from_utf16(&buf).ok().unwrap_or_default()
        } else if #[cfg(target_os = "linux")] {
            let name = hostname::get().unwrap_or_default();
            name.into_string().unwrap_or_default()
        } else {
            "Unknown device".into()
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(serde::Deserialize, Debug)]
#[serde(rename = "Win32_SystemEnclosure")]
#[serde(rename_all = "PascalCase")]
struct SystemEnclosure {
    chassis_types: Vec<u16>,
}

#[cfg(target_os = "windows")]
impl IntoIterator for SystemEnclosure {
    type Item = u16;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.chassis_types.into_iter()
    }
}

pub(crate) fn device_type() -> DeviceType {
    cfg_if! {
        if #[cfg(target_os = "ios")] {
            use objc2_ui_kit::UIDevice;
            use objc2_ui_kit::UIUserInterfaceIdiom;

            dispatch2::run_on_main(|mtm| {
                let ui_device = UIDevice::currentDevice(mtm);
                match ui_device.userInterfaceIdiom() {
                    UIUserInterfaceIdiom::TV |
                    UIUserInterfaceIdiom::CarPlay |
                    UIUserInterfaceIdiom::Pad |
                    UIUserInterfaceIdiom::Vision => DeviceType::Tablet,
                    UIUserInterfaceIdiom::Phone => DeviceType::Phone,
                    UIUserInterfaceIdiom::Mac => DeviceType::Laptop,
                    _ => DeviceType::Unknown,
                }
            })
        } else if #[cfg(target_os = "macos")] {
            // TODO: figure out if we can determine we're running on a laptop vs a desktop.
            DeviceType::Laptop
        } else if #[cfg(target_os = "windows")] {
            use wmi::{COMLibrary, WMIConnection};

            // COM/WMI are crucial. If these are not available, the app will not run.
            #[allow(clippy::expect_used)]
            let wmi_con = WMIConnection::new(COMLibrary::new().expect("Should have been to init COM!")).expect("Should have been able to connect to WMI!");
            // Expect usage: The query is always valid.
            #[allow(clippy::expect_used)]
            let system_enclosures: Vec<SystemEnclosure> = wmi_con.query().expect("Should have been able to query WMI");

            for chassis_type in system_enclosures.into_iter().flatten().collect::<Vec<u16>>() {
                match chassis_type {
                    // Tablet Types
                    30 => return DeviceType::Tablet,        // Tablet
                    11 => return DeviceType::Tablet,        // Hand Held (often considered tablet or PDA)

                    // Laptop Types
                    9 | 10 | 14 => return DeviceType::Laptop, // Laptop, Notebook, Sub Notebook
                    8 => return DeviceType::Laptop,         // Portable (generic, usually laptop)
                    16 => return DeviceType::Laptop,        // Lunch Box (specialized portable, often laptop-like)
                    31 => return DeviceType::Laptop,        // Convertible (Laptop/Tablet hybrid, primarily laptop usage mode)
                    32 => return DeviceType::Laptop,        // Detachable (Laptop/Tablet hybrid, primarily laptop usage mode)

                    // Desktop Types
                    3 => return DeviceType::Desktop,         // Desktop
                    4 => return DeviceType::Desktop,         // Low Profile Desktop
                    5 => return DeviceType::Desktop,         // Pizza Box
                    6 => return DeviceType::Desktop,         // Mini Tower
                    7 => return DeviceType::Desktop,         // Tower
                    13 => return DeviceType::Desktop,        // All in One (Desktop type)
                    15 => return DeviceType::Desktop,        // Space-Saving Desktop
                    35 => return DeviceType::Desktop,        // Mini PC

                    // Other codes are not mapped to Desktop, Laptop, or Tablet
                    _ => continue,
                }
            }
            DeviceType::Unknown
        } else if #[cfg(target_os = "linux")] {
            DeviceType::Unknown
        } else {
            DeviceType::Unknown
        }
    }
}
