# TrustToken Generation Tool (`generate_token`)

This tool generates a serialized TrustToken structure based on provided command-line arguments and an Ed25519 private key.

## Prerequisites

*   Rust and Cargo installed.
*   OpenSSL command-line tool (for key generation).

## 1. Generating the Signing Key

The tool requires an Ed25519 private key in PKCS#8 PEM format. You can generate one using `openssl`:

```bash
# Generate an Ed25519 private key and save it in PKCS#8 PEM format
openssl genpkey -algorithm Ed25519 -out /tmp/subject_private.pem

# (Optional) Extract the public key for verification or other purposes
openssl pkey -in /tmp/subject_private.pem -pubout -out /tmp/subject_public.pem
```

## 2. Building the Tool (optional)

Navigate to the root directory of the `trust_token` crate and build the binary:

```bash
cargo build --bin generate_token
```

The executable will be located at `${CARGO_TARGET_DIR}/debug/generate_token` (or `${CARGO_TARGET_DIR}/target/release/generate_token` if built with `--release`).

## 3. Running the Tool

Run the tool with `cargo run` by navigating to the generate_token directory.

The tool accepts several command-line arguments to define the TrustToken's properties:

```bash
cargo run -- --key-pem <KEY_PEM> --issuer-hash <ISSUER_HASH> --exp <EXP> --iat <IAT> --policy <POLICY>
```

**Arguments:**

*   `--key-pem <FILE>`: **Required.** Path to the issuer's Ed25519 private key PEM file (PKCS#8 format generated in Step 1).
*   `--subject-public-key <HEX>`: **Required.** Subject's public key (Ed25519, 32 bytes hex encoded). Example: `112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00`
*   `--issuer-hash <HEX>`: **Required.** The issuer hash (9 bytes, hex encoded). Example: `010203040506070809`.
*   `--exp <TIMESTAMP>`: **Required.** Expiration timestamp as Unix epoch seconds (e.g., `1735689600` for 2025-01-01 00:00:00 UTC).
*   `--iat <TIMESTAMP>`: **Required.** Issuance timestamp as Unix epoch seconds (e.g., `1704067200` for 2024-01-01 00:00:00 UTC).
*   `--policy <POLICY>`: **Required.** The TrustToken policy type. Must be one of:
    *   `VerifiedIdentity`
    *   `VerifiedIdentityStrong`
    *   `VerifiedDevice`
    *   `VerifiedDeviceStrong`
*   `--encrypted-identity-hash <HEX>`: *Optional.* The encrypted identity hash (up to 64 bytes, hex encoded).
*   `-o, --output <FILE>`: *Optional.* Output file path for the serialized CBOR data. If omitted, the output is written to standard output (stdout).
*   `-h, --help`: Print help information.
*   `-V, --version`: Print version information.

## 4. Examples

**Example 1: Generate a TrustToken and save to a file**

```bash
cargo run -- \
    --key-pem /tmp/subject_private.pem \
    --subject-public-key "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00" \
    --issuer-hash "010203040506070809" \
    --exp 1735689600 \
    --iat 1704067200 \
    --policy VerifiedIdentity \
    --encrypted-identity-hash "aabbccddeeff001121123344556677889119aabbccddeeff00112233cddeeff0" \
    --output /tmp/token.cbor

# Output: Serialized TrustToken written to: token.cbor
```

**Example 2: Generate a TrustToken without an encrypted identity hash and print to stdout**

```bash
cargo run -- \
    --key-pem /tmp/subject_private.pem \
    --subject-public-key "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00" \
    --issuer-hash "010203040506070809" \
    --exp 1735689600 \
    --iat 1704067200 \
    --policy VerifiedDeviceStrong
```
## 5. Viewing the Generated CBOR

You can use the `cbor-diag-cli` tool to inspect the contents of the generated CBOR file.

### Installing `cbor-diag-cli`

```bash
cargo install cbor-diag-cli
```

### Viewing the file

View the file generated from Example 1.

```bash
cbor-diag < /tmp/token.cbor
```

Modified version of Example 2 to view the CBOR file directly

```bash
cargo run -- \
    --key-pem /tmp/subject_private.pem \
    --subject-public-key "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00" \
    --issuer-hash "010203040506070809" \
    --exp 1735689600 \
    --iat 1704067200 \
    --policy VerifiedDeviceStrong \
    | cbor-diag
```
