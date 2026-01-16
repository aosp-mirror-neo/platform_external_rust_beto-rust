// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Binary to generate a serialized TrustToken CBOR file.

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use coset::CborSerializable;
use crypto_provider::ed25519::RawPrivateKeyPermit;
use crypto_provider_default::CryptoProviderImpl;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::SigningKey;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;
use trust_token::{
    IntermediateKeyIdentifier, PrivateKey, PublicKey, TrustTokenBuilder, TrustTokenPolicy,
};

#[allow(dead_code)] // TODO: remove dead code
const PUBLIC_KEY_SIZE: usize = 32; // Ed25519 public key
const ENCRYPTED_IDENTITY_HASH_SIZE: usize = 32;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the issuer's Ed25519 private key PEM file (PKCS#8 format).
    #[arg(long, value_name = "SIGNING_KEY_FILE")]
    key_pem: PathBuf,

    /// Subject's public key (Ed25519, 32 bytes hex encoded).
    #[arg(long, value_name = "HEX", value_parser = parse_hex::<PUBLIC_KEY_SIZE>)]
    subject_public_key: [u8; PUBLIC_KEY_SIZE],

    /// Issuer hash (12 bytes base64-url encoded).
    #[arg(long, value_name = "HEX", value_parser = parse_hex::<9>)]
    issuer_hash: [u8; 9],

    /// Expiration timestamp (Unix epoch seconds).
    #[arg(long, value_name = "TIMESTAMP")]
    exp: u32,

    /// Issuance timestamp (Unix epoch seconds).
    #[arg(long, value_name = "TIMESTAMP")]
    iat: u32,

    /// TrustToken policy type.
    #[arg(long, value_parser = parse_policy)]
    policy: TrustTokenPolicy,

    /// Optional encrypted identity hash (hex encoded).
    #[arg(long, value_name = "HEX", value_parser = parse_hex::<ENCRYPTED_IDENTITY_HASH_SIZE>)]
    encrypted_identity_hash: Option<[u8; ENCRYPTED_IDENTITY_HASH_SIZE]>,

    /// Output file path for the serialized CBOR data (defaults to stdout).
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn parse_policy(s: &str) -> Result<TrustTokenPolicy, String> {
    match s {
        "VerifiedIdentity" => Ok(TrustTokenPolicy::VerifiedIdentity),
        "VerifiedIdentityStrong" => Ok(TrustTokenPolicy::VerifiedIdentityStrong),
        "VerifiedDevice" => Ok(TrustTokenPolicy::VerifiedDevice),
        "VerifiedDeviceStrong" => Ok(TrustTokenPolicy::VerifiedDeviceStrong),
        _ => Err(format!("Invalid policy type: {}", s)),
    }
}

// Helper function to parse hex string into an array
fn parse_hex<const N: usize>(hex_str: &str) -> Result<[u8; N]> {
    let bytes = hex::decode(hex_str).map_err(|e| anyhow!("Invalid hex: {}", e))?;
    let arr: [u8; N] = bytes.try_into().map_err(|original: Vec<u8>| {
        anyhow!("Incorrect length: expected {} bytes, got {}", N, original.len())
    })?;
    Ok(arr)
}

fn main() -> Result<()> {
    let args = Args::parse();

    let issuer = IntermediateKeyIdentifier::new(args.issuer_hash);
    // --- Create TrustToken ---
    let mut builder = TrustTokenBuilder::new()
        .issuer(issuer)
        .expiration_time(trust_token::Seconds(args.exp))
        .issued_at(trust_token::Seconds(args.iat))
        .policy(args.policy)
        .public_key(
            PublicKey::from_bytes::<CryptoProviderImpl>(&args.subject_public_key)
                .expect("the key should be valid"),
        );
    if let Some(hash) = args.encrypted_identity_hash {
        builder = builder.encrypted_identity_hash(trust_token::EncryptedIdentityHash(hash));
    }
    let token = builder
        .build::<CryptoProviderImpl>()
        .map_err(|e| anyhow!("Failed to build token: {:?}", e))?;

    // --- Sign TrustToken ---
    let pem_content = fs::read_to_string(&args.key_pem).context("Read key-pem")?;
    // crypto_provider doesn't support decoding PEM yet.
    let signing_key = SigningKey::from_pkcs8_pem(&pem_content)
        .map_err(|e| anyhow!("Failed to parse key PEM: {}", e))
        .and_then(|key| {
            PrivateKey::from_bytes(key.as_bytes(), &RawPrivateKeyPermit::default())
                .map_err(|e| anyhow!("Invalid key: {}", e))
        })?;
    let signed_token = token
        .sign::<CryptoProviderImpl>(&signing_key)
        .map_err(|e| anyhow!("Failed to sign token: {:?}", e))?;

    eprintln!("Debug signed_token: {signed_token:?}");

    // Serialize the signed TrustToken (CoseSign1) to CBOR bytes
    let cbor_bytes =
        signed_token.to_vec().map_err(|e| anyhow!("Failed to serialize token to CBOR: {:?}", e))?;

    eprintln!(
        "Generated TrustToken CBOR ({} bytes):\n{}\n",
        cbor_bytes.len(),
        hex::encode(&cbor_bytes)
    );

    match args.output {
        Some(path) => {
            fs::write(&path, &cbor_bytes)?;
            eprintln!("Serialized TrustToken written to: {}", path.display());
        }
        None => {
            stdout().write_all(&cbor_bytes)?;
        }
    }

    Ok(())
}
