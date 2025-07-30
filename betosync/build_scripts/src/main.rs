// Copyright 2023 Google LLC
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

//! Main build script for the betosync project, used for both local development and in CI.
//!
//! Usage:
//! ```sh
//! cargo run -p build_scripts -- run-default-checks
//! ```

#![allow(missing_docs)]

use clap::Parser as _;
use cmd_runner::{
    cargo_workspace::{CargoOptions, CargoWorkspaceSubcommand, FormatterOptions},
    fuzzers::run_workspace_fuzz_targets,
    license_checker::LicenseSubcommand,
};
use env_logger::Env;
use license::LICENSE_CHECKER;
use log::info;
use std::{env, path};
use xshell::{cmd, Shell};

mod license;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let cli: Cli = Cli::parse();

    let build_script_dir = path::PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("Must be run via Cargo to establish root directory"),
    );
    let root_dir = build_script_dir
        .parent()
        .expect("Parent dir from build_script crate should exist");

    match cli.subcommand {
        Subcommand::RunDefaultChecks(ref check_options) => {
            run_default_checks(root_dir, check_options)?;
            print!(concat!(
                "Congratulations, the default checks passed. Since you like quality, here are\n",
                "some more checks you may like:\n",
                "    cargo run -p build_scripts -- run-rust-fuzzers\n",
            ));
        }
        Subcommand::VerifyCi { ref check_options } => verify_ci(root_dir, check_options)?,
        Subcommand::RunAllChecks { ref check_options } => run_all_checks(root_dir, check_options)?,
        Subcommand::CleanEverything => clean_everything(root_dir)?,
        Subcommand::CargoWorkspace(subcommand) => {
            let sh = xshell::Shell::new()?;
            sh.change_dir(root_dir);
            subcommand.run(&sh, "betosync", ["--features=enable-required-features"])?
        }
        Subcommand::RunRustFuzzers => run_workspace_fuzz_targets(root_dir)?,
        Subcommand::License(license_subcommand) => {
            license_subcommand.run(&LICENSE_CHECKER, root_dir)?
        }
    }

    Ok(())
}

fn check_java(sh: &Shell) -> anyhow::Result<()> {
    let _guard = sh.push_dir("submerge/submerge_java");
    cmd!(sh, "./gradlew :lib:test --rerun-tasks").run()?;
    Ok(())
}

#[allow(clippy::if_same_then_else)]
pub fn verify_ci(root: &path::Path, check_options: &crate::CheckOptions) -> anyhow::Result<()> {
    if cfg!(target_os = "linux") {
        run_default_checks(root, check_options)?;
    } else if cfg!(target_os = "macos") {
        license::LICENSE_CHECKER.check(root)?;
        let sh = xshell::Shell::new()?;
        sh.change_dir(root);
        check_options.cargo_options.check_workspace(
            &sh,
            "betosync",
            ["--features=enable-required-features"],
        )?;
    } else if cfg!(windows) {
        license::LICENSE_CHECKER.check(root)?;
        let sh = xshell::Shell::new()?;
        sh.change_dir(root);
        check_options.cargo_options.check_workspace(
            &sh,
            "betosync",
            ["--features=enable-required-features"],
        )?;
    } else {
        anyhow::bail!("Unsupported OS for running CI!")
    }
    Ok(())
}

/// Runs checks to ensure lints are passing and all targets are building
pub fn run_all_checks(root: &path::Path, check_options: &CheckOptions) -> anyhow::Result<()> {
    run_default_checks(root, check_options)?;
    run_workspace_fuzz_targets(root)?;
    Ok(())
}

/// Runs default checks that are suitable for verifying a local change.
pub fn run_default_checks(root: &path::Path, check_options: &CheckOptions) -> anyhow::Result<()> {
    license::LICENSE_CHECKER.check(root)?;
    let sh = xshell::Shell::new()?;
    sh.change_dir(root);
    check_options.cargo_options.check_workspace(
        &sh,
        "betosync",
        ["--features=enable-required-features"],
    )?;
    check_java(&sh)?;
    Ok(())
}

pub fn clean_everything(root: &path::Path) -> anyhow::Result<()> {
    let sh = Shell::new()?;
    sh.change_dir(root);
    cmd!(sh, "cargo clean").run()?;
    sh.change_dir(root.join("submerge/submerge_java"));
    cmd!(sh, "./gradlew :tests --rerun-tasks").run()?;
    Ok(())
}

#[derive(clap::Parser)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Subcommand,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Subcommand {
    /// Runs all the checks that CI runs
    VerifyCi {
        #[command(flatten)]
        check_options: CheckOptions,
    },
    /// Runs all available checks, including ones that are not normally run on CI
    RunAllChecks {
        #[command(flatten)]
        check_options: CheckOptions,
    },
    /// Runs the default set of checks suitable for verifying local changes.
    RunDefaultChecks(CheckOptions),
    /// Cleans the main workspace and all sub projects - useful if upgrading rust compiler version
    /// and need dependencies to be compiled with the same version
    CleanEverything,
    #[command(flatten)]
    CargoWorkspace(CargoWorkspaceSubcommand),
    /// Build and run pure Rust fuzzers for 10000 runs
    RunRustFuzzers,
    #[command(flatten)]
    License(LicenseSubcommand),
}

#[derive(clap::Args, Debug, Clone, Default)]
pub struct CheckOptions {
    #[command(flatten)]
    formatter_options: FormatterOptions,
    #[command(flatten)]
    cargo_options: CargoOptions,
}
