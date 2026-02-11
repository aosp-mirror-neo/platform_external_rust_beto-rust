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

//! Test certificates and keys for QUIC handshake verification.
//!
//! # Security Warning
//! **NEVER** use these keys in production. The private keys are hardcoded in the source
//! and are publicly visible. They are strictly for local testing and CI environments.
//!
//! # Certificate Generation
//!
//! The following OpenSSL commands were used to generate the constants below.
//!
//! ```bash
//! #### Step 1: Create a valid CA key and certificate
//! #### Give the CA a valid Common Name "Test CA".
//! # 1. Create CA key
//! openssl genrsa -out ca_key.pem 2048
//!
//! # 2. Create CA certificate (Self-signed) with a valid Subject
//! openssl req -x509 -new -nodes -key ca_key.pem -sha256 -days 3650 \
//!   -out ca_certificate.pem \
//!   -subj "/C=/ST=/L=/O=/CN=Test CA"
//!
//! #### Step 2: Create Server Key and CSR
//! # 3. Create Server private key
//! openssl genrsa -out server_key.pem 2048
//!
//! # 4. Create Server CSR
//! openssl req -new -key server_key.pem -out server_req.pem \
//!   -subj "/C=/ST=/L=/O=/CN=example.com"
//!
//! #### Step 3: Create Server Certificate with SAN
//! # 5. Create an extensions file
//! echo "subjectAltName=DNS:example.com" > server.ext
//!
//! # 6. Sign the certificate using the extensions file
//! openssl x509 -req -in server_req.pem -CA ca_certificate.pem -CAkey ca_key.pem \
//!   -CAcreateserial -out server_certificate.pem -days 3650 -sha256 \
//!   -extfile server.ext
//! ```

/// The server name used in the Subject Alternative Name (SAN) of the certificate.
pub(crate) const TEST_SERVER_NAME: &str = "example.com";

// ca_certificate.pem
#[allow(dead_code)]
pub(crate) const TEST_CA_CERT: &str = r#"
-----BEGIN CERTIFICATE-----
MIIDBTCCAe2gAwIBAgIUbaIwGvRMNqy6cGPrEBwIocnLScEwDQYJKoZIhvcNAQEL
BQAwEjEQMA4GA1UEAwwHVGVzdCBDQTAeFw0yNjAxMDIwNzU3MjNaFw0zNTEyMzEw
NzU3MjNaMBIxEDAOBgNVBAMMB1Rlc3QgQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IB
DwAwggEKAoIBAQD16YDlj3HgYgjgMHPKiajmrS2iVdg5BOeOp0TnetudbrBembK5
+YU4uAF6yQAG9CIkMBIwCX2HH51xxmZdWpqiYadF7tfSZnkzkQwuWVcmP4t74szs
aW4jpt1M3MjtWh68G6rgx31jINkryrh2cGw4RijPzTqr/pZFBDOe7AQr2+9FrCEd
3JzkMPnmOdmvQbJPjrrwKYAGjd7BFncvdFOytQsOGTNrgRGm69jcUqVJdB759PuJ
w1nuvz4CeMvmivwTrdFALdt1raInYM9ytKpjzsC8DbKl/iJasjbTa+lqp9HdKRtH
AaIQJ8gc532iWQUmAQs2fMQW3fpPIByYaRkRAgMBAAGjUzBRMB0GA1UdDgQWBBQx
Kby/NpvHfTBHXlLjVLEWFHnbSTAfBgNVHSMEGDAWgBQxKby/NpvHfTBHXlLjVLEW
FHnbSTAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQBlAreI2c3Q
bVRFSVc+TGK+P8VMtNzsTor+/PYvc/sgvsYsIzTI/jeVvJ7mbsxp4tVFX4VxI4Uf
Gpcwr5FC0A4YXDo4YXT0NDoTLUvU5GeCGZSIbSzsvHk0dkwxNGb6hemqPJmeCEee
03ZuaaiARtRcQfVr3FDy54RWyJTMt8J7/YiIXbAqriicAKksBG8p/4I2+ncWFvI2
a7WtNIIY9jfGEIcgQqniTX1q2uQ5za8DmbYwjLwVE8AcT8gebac9mmD+HpMUIepc
As+fn8AyufICEmpnRQY788DYA3EJp55hnp3p/Wia95WDIJBKmfnXHuhIAueJcg6W
YDOdR5nEULPK
-----END CERTIFICATE-----
"#;

// server_key.pem
pub(crate) const TEST_SERVER_KEY: &str = r#"
-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCmjkwwTUfefEbq
CtypnGETBRlkDTPlGKc2IWlDXJlypJ7kca4Hyb8/mYM/KcTUqeW6In8lNR4LhgdO
cqVomvMfJ53vxrgD9nhaeCNEKkM9mipwcenbm6qh/3cbxX8+h+2UfieKS0Vfo8e6
1BW/Fu31ifW4Ej6gRz5stI1rcWlleXfZsZNKH2Tg+NBlVLh79LotWTIaP6MeQ1gi
ysdBlsZz5sz4eFODHjAwG8QkUxI9zOSoKPwgCeIZ6gv4mtEaWOm3/veKzJAZY1hN
kfnD0X86s7TqduvFsvEbE/3S+bpIrl6Q8w0LfEhnqemTtUtv0+8Qbr77wmotVzZB
6qC2vuMNAgMBAAECggEATJmzRp+0RwnHH4qhU8xdBP9aXx6HWH0PIiEZpWZoGEbH
4O67iE9sCtcSerE+GkSNKB8HdNhKGiipIZYupr5kxFZXF9i/9uqbDZilN4tsY1+M
1yw8zsh4IIs6f2ZEkEC5roqaqsrVfBf6Gt3BpqHoDP5HKUIS1WoOtrYPOHPvckA9
kR4alFD4JHQSIOJH5nRZ6QOMbX9KJqGl2ZSVIOFEDM+OkONS4/GGyN5vdl9JPEm1
e8lmzPaJi7pJYPxICTKWzYHHWFRrAfFOqfyx+jbaAp4QhFpT+K2wYlYt0UOPNq7A
3WExmjPkqEKByisZnDV2K/33HvfEn1rgnrkULVPgLQKBgQDg+6E51cPeSW/kZhDj
B0rUeVNKmrxjBWEpnlVpGlGUyNvODP/FL74eIj4cMtB0lU3WG5qtq034Auanfndu
XbIx9ielKKZNgTXuUyAKTUCvotaPc+NRgHJ2XYeQyCai9yCxezOifBGOi2aL0QWj
eblk5cFf9JjeMj5uhBYgY/IvnwKBgQC9hJaqt0ftc7u5eoscxMentOk1tH88aWcd
1Zv/kRp22KoXWgFRnMwb648fg+v9DSTMnEc2K5PQC/1/RYHmdQvkL3gaeGd6ayjd
ZruolhyVyqMIg5tWXZjNS9NPiEtpyZ3Z9oivUc0so5xVl7OxPfUUnS8s0u/uUzYz
nqlsa3990wKBgC1ekQS4tub8qDPG4WDm95AhZ0qEKblOPGnIJDEtvYXje2/x7yI0
noI1JZFKLf9UnH2w02BySybYbusX9b2r52Nr4FLN/7thcx68ZJmvOnwwulPzasPk
wzBbbBsmQaRbcZxY2/DIVEIKmoNXg2uUAG74ZiaTjMBCIZTnzg56GTzBAoGAXnrb
IgB9O0OXMsCE2bLAVjs6iCuTwupJHfZ/7sjICLe7FCkqvyUMHF8y2uNJor4Kunau
ibLv3agcmHZubBWt9/QyS//6lm0V0kqFA2bSmVqMLJJvvn6qzq4eimfIMNTyfNcL
DVwajUaDiag3TQU2STCTdCU0t/uD6fgPPp3Y0UECgYEAlGuZeTj24+ani3Ki1G/G
T4zkpmxsGer/Sqyc4dLtGJN7DUXQSJ57re+uHQSn2e9yMwGkGP8g61kEeGV/GlSa
CIAxP4Vq5Ya+U4wJ8rtrm6oyw/DHfyNlxpial91zNc/Bqbjk1tXbBLd7fJZq8DeO
zkXkNH4TXMsO4uGQGDMtI5A=
-----END PRIVATE KEY-----
"#;

// server_certificate.pem
pub(crate) const TEST_SERVER_CERT: &str = r#"
-----BEGIN CERTIFICATE-----
MIIDEDCCAfigAwIBAgIUffaSh/9yQU6uqPQtwfn261MaasswDQYJKoZIhvcNAQEL
BQAwEjEQMA4GA1UEAwwHVGVzdCBDQTAeFw0yNjAxMDIwNzU3MjNaFw0zNTEyMzEw
NzU3MjNaMBYxFDASBgNVBAMMC2V4YW1wbGUuY29tMIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEApo5MME1H3nxG6grcqZxhEwUZZA0z5RinNiFpQ1yZcqSe
5HGuB8m/P5mDPynE1KnluiJ/JTUeC4YHTnKlaJrzHyed78a4A/Z4WngjRCpDPZoq
cHHp25uqof93G8V/PoftlH4niktFX6PHutQVvxbt9Yn1uBI+oEc+bLSNa3FpZXl3
2bGTSh9k4PjQZVS4e/S6LVkyGj+jHkNYIsrHQZbGc+bM+HhTgx4wMBvEJFMSPczk
qCj8IAniGeoL+JrRGljpt/73isyQGWNYTZH5w9F/OrO06nbrxbLxGxP90vm6SK5e
kPMNC3xIZ6npk7VLb9PvEG6++8JqLVc2Qeqgtr7jDQIDAQABo1owWDAWBgNVHREE
DzANggtleGFtcGxlLmNvbTAdBgNVHQ4EFgQUXuKipbQqaOuEI9o0AfEAblKulOEw
HwYDVR0jBBgwFoAUMSm8vzabx30wR15S41SxFhR520kwDQYJKoZIhvcNAQELBQAD
ggEBAH9EjCtLvJbIG+TwvA9GkBR60OV32pPO59hU0NqgPCwcXzj6dqIzVKtsVj5J
4kYGXFbzjCubzKMfao+Ukq9L/hWfolTYToxd4tKDcHJSEZCSyuE9vvseoTzmKt/V
Ov7oO5jUVjUF1VpSqkBH/SOpjvESXt51kJvz2Eis72hyAEf+sMpacF0jJ9O2DTll
duVe74EC7sxqGUeKo/ZUTBfd7xuIKS76LgcSul7li6wgpFYnL3V0CZnWXyIr4ohJ
b2cFKGfbtZpRdV4zPqQPCdSOgWjSTS6A9AAWkE+DEkeR4dTsYPQAs9wM3BQUuu+3
1srUklQFWb73GAvP/12MWXkqDjY=
-----END CERTIFICATE-----
"#;
