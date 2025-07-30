# np_adv Java FFI

This package defines a Java interface for the `np_adv` library.

This package is split into directories:
* `java/` - The Java library with `native` methods.
* `src/` - The Rust library that implements the `native` Java methods.
    - `classes/` - Contains Rust files matching Java classes from `java/`. Code in these files will implement the `native` methods for the associated Java class.
* `test/` - Java tests that test the Java library and underlying native code.

The build is controlled by the following files:
* `build.gradle.kts` - Defines the builds for `java/` and `test/` along with Java dependencies for the Java library.
    - `settings.gradle.kts` - Defines where local Java dependencies are located.
* `Cargo.toml` - Defines the Rust .so binary that is loaded by the Java library.

This library defines the following build_scripts actions:
* `run-np-java-ffi-tests` - Builds the package and runs the Java test suite.

Additional configs for external use:
* `AndroidManifest.xml` - Defines the Android `minSdkVersion` required by the Java library.
* `used_by_native.pgcfg` - Defines proguard exclusions for the `@UsedByNative` annotation, so that native code can access Java structures.
