# Gemini Development Guidelines for `identity_manager`

This document outlines the core principles and architectural layout of the `identity_manager` crate to ensure efficient and accurate development.

## 1. Core Principles

*   **Start with the Public API:** For any task, the initial step is to read `lib.rs` and `api.rs`. The `api.rs` file defines the core traits that constitute the crate's public-facing contract: `IdentityManager`, `Codec`, and `Authenticator`.
*   **Feature-Driven Architecture:** The crate's functionality and exposed API change significantly based on the `encrypted_identity` feature flag.
    *   **With `encrypted_identity`:** Provides full identity management, credential storage, and encoding/decoding of both public and encrypted advertisements. It implements the `IdentityManager`, `Codec`, and `Authenticator` traits.
    *   **Without `encrypted_identity`:** Provides a lightweight implementation focused on encoding/decoding public-only advertisements. It only implements the `Codec` trait.
*   **Trait-Driven Development:** The primary implementation (`manager_impl::IdentityManagerImpl`) is a composition of components that fulfill the contracts of the main API traits. When analyzing or implementing code, focus on these traits and their dependencies.

## 2. Architectural Overview

The `identity_manager` crate is responsible for managing device identity, credentials, and the encoding/decoding of advertisements. Its architecture is centered around the `IdentityManagerImpl` struct, which has two variants depending on the `encrypted_identity` feature.

*   **`IdentityManagerImpl` (`manager_impl.rs` - with `encrypted_identity` feature)**
    *   This is the primary, full-featured implementation.
    *   It implements the `api::IdentityManager`, `api::Codec`, and `api::Authenticator` traits.
    *   **Depends on:**
        *   `CredentialManager`: The core component for handling cryptographic credentials.
        *   `PreferencesManager`: For managing device-specific information (name, type, etc.).
        *   `IdentityProvider`: A trait from the `identity_provider` crate, representing external sources of identity (e.g., Google accounts).

*   **`IdentityManagerImpl` (`public_manager_impl.rs` - without `encrypted_identity` feature)**
    *   A lightweight implementation for public advertisements only.
    *   It implements only the `api::Codec` trait.
    *   **Depends on:**
        *   `PreferencesManager`: To get device info for encoding public advertisements.

### Key Components (primarily for `encrypted_identity` feature)

*   **`CredentialManager` (`credentials/credential_manager.rs`)**
    *   Manages the lifecycle of local (self) and peer (remote) credentials.
    *   It is responsible for generating, storing, and retrieving credentials for advertisement encoding/decoding and authentication.
    *   **Depends on:**
        *   `CredentialsDatabase`: For persistent storage of credentials.
        *   `KeyStore`: For secure handling of cryptographic keys.
        *   `up_adv`: For building the `CredentialBook` used in advertisement processing.
        *   `credential_proto`: For the credential data structures.

*   **`Codec` Implementation (`codec/codec_impl.rs`)**
    *   Handles the logic for `encode_with_visibility` and `decode_with_visibility` from the `api::Codec` trait.
    *   It uses the `up_adv` crate for the underlying advertisement format serialization and deserialization.
    *   **Depends on:**
        *   `CredentialManager`: To fetch credentials for encrypting and decrypting advertisement sections.

*   **Storage Layer (`identity_storage/`)**
    *   `PreferencesManager`: Manages non-sensitive device metadata (e.g., device name, type) in `preferences.json`.
    *   `CredentialsDatabase`: Manages sensitive cryptographic credentials (self and peer) in `credentials.json`.
    *   Both components depend on the `storage` crate for the underlying key-value store implementation.
