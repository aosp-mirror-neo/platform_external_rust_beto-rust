/*
 * Copyright 2025 Google LLC
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

import net.ltgt.gradle.errorprone.errorprone

plugins {
  `java-library`
  id("net.ltgt.errorprone") version "4.0.0"
}

repositories {
  mavenCentral()
  google()
}

dependencies {
  compileOnly("org.checkerframework:checker-qual:3.47.0")
  compileOnly("com.google.errorprone:error_prone_annotations:2.29.2")
  errorprone("com.google.errorprone:error_prone_core:2.29.2")

  implementation("com.google.guava:guava:31.1-jre")

  testImplementation("com.google.truth:truth:1.1.4")
  testImplementation("org.junit.jupiter:junit-jupiter:5.9.2")
}

java { toolchain { languageVersion = JavaLanguageVersion.of(17) } }

task<Exec>("cargo") { commandLine("cargo", "build", "--color=always", "--features=create") }

tasks.withType<JavaCompile> {
  dependsOn("cargo")
  options.errorprone {
    disable("DefaultCharset")
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
    error("MustBeClosedChecker")
  }
  options.compilerArgs.addAll(arrayOf("-Xlint:rawtypes,unchecked", "-Werror"))
}

sourceSets {
  test {
    resources {
      srcDirs("src/test/testdata")
    }
  }
}

tasks.test {
  systemProperty("java.library.path", "$projectDir/../../../target/debug")
  testLogging { showStandardStreams = true }
  jvmArgs = mutableListOf("-Xcheck:jni")
}
