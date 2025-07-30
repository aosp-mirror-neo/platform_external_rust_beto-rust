# build\_scripts User Guide

This project contains the scripts that are required to build, lint, test, and run projects in this repository. The scripts are also used by the CI workflows and define what gets run under CI.

Generally, the scripts are run with the following command template: `cargo run -p build_scripts -- $build_args...`. The `$build_args` are the arguments passed to the `build_scripts` binary.

The list of supported actions can be seen using: `cargo run -p build_scripts -- --help`

## Modifying build\_scripts

### Adding top-level actions

The top level of the tool is defined using [proc-macros][clap_derive] from the [`clap`][clap] crate. Our top level actions are defined in `enum SubCommand` in `main.rs`.

1. Create a function like `fn my_build_action(root: &Path)` that performs the action. The `root` argument will be the path to the root directory of the project.
1. Add an entry to `SubCommand` in `main.rs`. The doc comment will be the `--help` info. See the [clap documentation][clap_derive] on how to define additional arguments or flags.
1. Create a handler in `fn main()` that matches the new subcommand and calls the function defined in step 1.
1. Add any additional cleanup required by the action to `fn clean_everything(root: &Path)` in `main.rs`.
1. (*optional*) Consider adding the action to CI by adding a call to the function from `fn verify_ci` in `main.rs`.

[clap]: https://docs.rs/clap
[clap_derive]: https://docs.rs/clap/latest/clap/_derive/index.html

## Motivation

Cargo is not a suitable tool for managing our various build configurations nor is it meant to build non-Rust libraries. Since our repository contains binaries requiring various feature-sets and libraries in many non-Rust languages, we needed a common place to define how the project is built. This package is that common place. Additionally, by writing this in Rust we can run these scripts on multiple platforms without the need for additional external dependencies.
