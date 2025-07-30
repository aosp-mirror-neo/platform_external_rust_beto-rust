/*
 * Copyright 2023 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import net.ltgt.gradle.errorprone.errorprone;

plugins {
  `java-library`
  // For static analysis
  id("net.ltgt.errorprone") version "4.0.0"
}

java {
  // Gradle JUnit test finder doesn't support Java 21 class files.
  sourceCompatibility = JavaVersion.VERSION_1_9
  targetCompatibility = JavaVersion.VERSION_1_9
}

repositories {
  mavenCentral()
  google()
}

dependencies {
  // Static analysis annnotations
  implementation("androidx.annotation:annotation:1.6.0")
  implementation("com.google.errorprone:error_prone_core:2.28.0")
  implementation("org.checkerframework:checker-qual:3.45.0")

  // Local dependencies
  implementation(project(":cooperative_cleaner"))

  // JUnit Test Support
  testImplementation("junit:junit:4.13")
  testImplementation("com.google.truth:truth:1.1.4")
  testImplementation("com.google.code.gson:gson:2.10.1")
  testImplementation("org.mockito:mockito-core:5.+")
}

// Flattened directory layout
sourceSets {
  main {
    java {
      setSrcDirs(listOf("java"))
    }
  }
  test {
    java {
      setSrcDirs(listOf("test"))
    }
  }
}

tasks.test {
  useJUnit()
  jvmArgs = mutableListOf(
      // libnp_java_ffi.so
      // This is the Cargo workspace's build output directory
      "-Djava.library.path=$projectDir/../../target/debug",
      // ByteBuddy agent for mocks
      "-XX:+EnableDynamicAgentLoading"
  )
}

buildscript {
    repositories {
        mavenCentral()
    }
    dependencies {
        classpath("com.guardsquare:proguard-gradle:7.7.0") {
            exclude("com.android.tools.build")
        }
    }
}

// Task to take built test classes and package them into a .jar
// which may then be fed as input to proguard.
tasks.register<Jar>("jarTestClasses") {
    from("build/classes/java/test")
    archiveFileName.set("tests.jar")
    destinationDirectory.set(file("build/testjars"))
    dependsOn("compileTestJava")
}

// Task to obfuscate the library to np-java-ffi-proguarded.jar
tasks.register<proguard.gradle.ProGuardTask>("proguardForTests") {
    injars(listOf("build/libs/np-java-ffi.jar", "build/testjars/tests.jar"))
    outjars("build/libs/np-java-ffi-proguarded-tests.jar")
    configuration("proguarded_unit_tests.pgcfg")
    dependsOn("jarTestClasses")
    dependsOn("jar")
}

// Expands the proguarded .jar into a directory structure with
// .class files.
tasks.register<Copy>("unzipProguardedTestsJar") {
    from(zipTree("build/libs/np-java-ffi-proguarded-tests.jar"))
    into("build/libs/unzippedproguardedtests")
    dependsOn("proguardForTests")
}

tasks.register<Test>("proguardedTest") {
  useJUnit()
  jvmArgs = mutableListOf(
      // libnp_java_ffi.so
      // This is the Cargo workspace's build output directory
      "-Djava.library.path=$projectDir/../../target/debug",
      // ByteBuddy agent for mocks
      "-XX:+EnableDynamicAgentLoading"
  )
  // Ignore the usual built classes
  classpath -= files("build/classes/java/main")
  classpath -= files("build/classes/java/test")
  // Use the proguarded classes instead
  classpath += files("build/libs/unzippedproguardedtests")
  dependsOn("unzipProguardedTestsJar")
  // Exclude test-vector checks, since that
  // uses reflection, which won't play nice with proguard.
  exclude("**/*TestVectors*")
}

tasks.withType<JavaCompile>().configureEach {
  // Configure static analysis passes to match g3 if possible
  options.errorprone {
    error("CheckReturnValue")
    error("UnnecessaryStaticImport")
    error("WildcardImport")
    error("RemoveUnusedImports")
    error("ReturnMissingNullable")
    error("FieldMissingNullable")
    error("AnnotationPosition")
    error("CheckedExceptionNotThrown")
    error("NonFinalStaticField")
    error("InvalidLink")
    error("ThrowsUncheckedException")
  }
}
