//! GCP Cloud KMS signing support.
//!
//! Lets governance commands sign with an Ed25519 key held in Cloud KMS
//! (algorithm `EC_SIGN_ED25519`) instead of a local keypair file, so the
//! private key never touches disk. Anywhere the CLI accepts a keypair path
//! for a single-sig authority, a `kms://` URI can be passed instead:
//!
//! ```text
//! kms://projects/<p>/locations/<l>/keyRings/<r>/cryptoKeys/<k>/cryptoKeyVersions/<v>?pubkey=<BASE58_ADDRESS>
//! ```
//!
//! The URI names the exact crypto-key version and the Solana address the key
//! is expected to have; every returned signature is verified against that
//! address (fails closed on mismatch). Authentication uses Application
//! Default Credentials (`gcloud auth application-default login`, or
//! `GOOGLE_APPLICATION_CREDENTIALS`).
//!
//! [`KmsSigner`] adapts solana-keychain's async `GcpKmsSigner` to the sync
//! [`solana_sdk::signer::Signer`] trait used across the CLI, via a dedicated
//! tokio runtime.

use crate::cli::output::{emit, subfield, OutputMode, Render};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use google_cloud_kms_v1::client::KeyManagementService;
use google_cloud_kms_v1::model::crypto_key_version::CryptoKeyVersionAlgorithm;
use solana_keychain::{GcpKmsSigner, SolanaSigner};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::signer::{Signer, SignerError};
use std::fs;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// URI scheme marking a CLI signer argument as a Cloud KMS key rather than a
/// keypair file path.
pub const KMS_URI_SCHEME: &str = "kms://";

const KMS_URI_FORMAT: &str = "kms://projects/<p>/locations/<l>/keyRings/<r>/cryptoKeys/<k>/cryptoKeyVersions/<v>?pubkey=<BASE58_ADDRESS>";

/// The dedicated runtime that drives the async KMS client from sync CLI code.
///
/// Must be a runtime this module owns: `Handle::current().block_on()` from a
/// thread already inside a runtime would panic. Current-thread flavor — the
/// CLI only ever makes a handful of sequential `block_on` calls, so worker
/// threads would sit idle.
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to start tokio runtime")
    })
}

/// Sync [`Signer`] over a key held in GCP Cloud KMS.
///
/// Only `sign_message` is bridged (raw PureEdDSA over the transaction message
/// bytes) — keychain's own `sign_transaction` flow is not used. Signing does
/// a network round-trip per call, so it can fail like any RPC: callers must
/// use `try_sign`, never the panicking `sign`.
pub struct KmsSigner {
    inner: GcpKmsSigner,
}

impl KmsSigner {
    /// Build a signer from a `kms://` URI, verifying the key before any
    /// transaction is built.
    pub fn from_uri(uri: &str) -> eyre::Result<Self> {
        let (key_name, pubkey_b58) = parse_kms_uri(uri)?;
        runtime().block_on(async {
            let client = KeyManagementService::builder().build().await.map_err(|e| {
                eyre::eyre!(
                    "Failed to create KMS client: {} (authentication uses Application Default \
                     Credentials — run `gcloud auth application-default login`, or set \
                     GOOGLE_APPLICATION_CREDENTIALS)",
                    e
                )
            })?;
            Self::with_client(client, key_name, pubkey_b58).await
        })
    }

    /// Preflight the key with one `getPublicKey` round-trip, then construct
    /// the signer: verifies the key is accessible (surfacing the real API
    /// error on 403/404 — keychain's own signing errors carry no detail),
    /// that its algorithm is `EC_SIGN_ED25519`, and that its actual address
    /// matches the one declared in the URI. The address check catches a
    /// wrong `?pubkey=` here, with the actual address in the message,
    /// instead of as an opaque verification failure at signing time.
    ///
    /// The one thing this cannot preflight is the `useToSign` permission
    /// itself: a principal that can view the public key but not sign still
    /// fails at signing time.
    async fn with_client(
        client: KeyManagementService,
        key_name: String,
        pubkey_b58: String,
    ) -> eyre::Result<Self> {
        let public_key = client
            .get_public_key()
            .set_name(&key_name)
            .send()
            .await
            .map_err(|e| eyre::eyre!("Cannot access KMS key {}: {}", key_name, e))?;
        if public_key.algorithm != CryptoKeyVersionAlgorithm::EcSignEd25519 {
            eyre::bail!(
                "KMS key {} has algorithm {:?}; it must be EC_SIGN_ED25519",
                key_name,
                public_key.algorithm
            );
        }
        let actual = address_from_pem(&public_key.pem).map_err(|e| {
            eyre::eyre!("Failed to parse public key of KMS key {}: {}", key_name, e)
        })?;
        if actual.to_string() != pubkey_b58 {
            eyre::bail!(
                "The pubkey in the KMS URI ({}) does not match the key's actual address ({})",
                pubkey_b58,
                actual
            );
        }
        let inner = GcpKmsSigner::with_client(client, key_name, pubkey_b58)
            .map_err(|e| eyre::eyre!("Failed to initialize GCP KMS signer: {}", e))?;
        Ok(Self { inner })
    }
}

impl Signer for KmsSigner {
    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        Ok(self.inner.pubkey())
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        runtime()
            .block_on(self.inner.sign_message(message))
            .map_err(|e| SignerError::Custom(format!("GCP KMS: {}", e)))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

/// Split a `kms://` URI into the crypto-key-version resource name and the
/// expected base58 address. Both parts are validated for shape here so errors
/// point at the URI, not at a failed API call later.
fn parse_kms_uri(uri: &str) -> eyre::Result<(String, String)> {
    let rest = uri
        .strip_prefix(KMS_URI_SCHEME)
        .ok_or_else(|| eyre::eyre!("Not a KMS URI '{}': expected {}", uri, KMS_URI_FORMAT))?;
    let (key_name, query) = rest.split_once('?').ok_or_else(|| {
        eyre::eyre!(
            "KMS URI '{}' is missing '?pubkey=<BASE58_ADDRESS>': expected {}",
            uri,
            KMS_URI_FORMAT
        )
    })?;
    let pubkey_b58 = query.strip_prefix("pubkey=").ok_or_else(|| {
        eyre::eyre!(
            "KMS URI query '{}' must be 'pubkey=<BASE58_ADDRESS>': expected {}",
            query,
            KMS_URI_FORMAT
        )
    })?;
    if !key_name.starts_with("projects/") || !key_name.contains("/cryptoKeyVersions/") {
        eyre::bail!(
            "KMS key name '{}' must be a full crypto-key-version resource name: expected {}",
            key_name,
            KMS_URI_FORMAT
        );
    }
    pubkey_b58
        .parse::<Pubkey>()
        .map_err(|e| eyre::eyre!("Invalid pubkey '{}' in KMS URI: {}", pubkey_b58, e))?;
    Ok((key_name.to_string(), pubkey_b58.to_string()))
}

/// DER prefix of an Ed25519 SubjectPublicKeyInfo (RFC 8410): a 44-byte SPKI
/// whose final 32 bytes are the raw public key.
const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Convert a Cloud KMS Ed25519 public key PEM (as written by `gcloud kms keys
/// versions get-public-key`) to its Solana address.
///
/// Rejects non-Ed25519 keys rather than silently deriving a wrong address
/// from the trailing bytes of some other key type.
pub fn address_from_pem(pem: &str) -> eyre::Result<Pubkey> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .map(str::trim)
        .collect();
    let der = BASE64
        .decode(body.as_bytes())
        .map_err(|e| eyre::eyre!("Failed to base64-decode PEM body: {}", e))?;
    if der.len() != 44 || der[..12] != ED25519_SPKI_PREFIX {
        eyre::bail!(
            "Not an Ed25519 public key (expected a 44-byte RFC 8410 SubjectPublicKeyInfo). \
             The KMS key must use algorithm EC_SIGN_ED25519."
        );
    }
    let key: [u8; 32] = der[12..].try_into().expect("length checked above");
    Ok(Pubkey::from(key))
}

#[derive(serde::Serialize)]
pub struct KmsAddressView {
    address: String,
}

impl Render for KmsAddressView {
    fn to_text(&self) -> String {
        let mut out = String::from("✅ KMS public key parsed\n");
        out.push_str(&subfield("Address", &self.address));
        out
    }
}

/// `kms address` — print the Solana address of a Cloud KMS Ed25519 public
/// key PEM, for constructing the `kms://` URI and funding/authorizing the key.
pub fn show_address(pem_path: String, mode: OutputMode) -> eyre::Result<()> {
    let pem = fs::read_to_string(&pem_path)
        .map_err(|e| eyre::eyre!("Failed to read public key PEM from {}: {}", pem_path, e))?;
    let address = address_from_pem(&pem)
        .map_err(|e| eyre::eyre!("Failed to parse public key PEM {}: {}", pem_path, e))?;
    emit(
        &KmsAddressView {
            address: address.to_string(),
        },
        mode,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const KEY_NAME: &str = "projects/test-project/locations/us-central1/keyRings/test-ring/cryptoKeys/test-key/cryptoKeyVersions/1";

    // RFC 8410 test vector: this SPKI decodes to the all-A's example key,
    // whose base58 form is the well-known address below.
    const RFC8410_PEM: &str = "-----BEGIN PUBLIC KEY-----\n\
                               MCowBQYDK2VwAyEAO2onvM62pC1io6jQKm8Nc2UyFXcd4kOmOsBIoYtZ2ik=\n\
                               -----END PUBLIC KEY-----\n";
    const RFC8410_ADDRESS: &str = "4zvwRjXUKGfvwnParsHAS3HuSVzV5cA4McphgmoCtajS";

    #[test]
    fn parse_kms_uri_roundtrip() {
        let uri = format!("kms://{}?pubkey={}", KEY_NAME, RFC8410_ADDRESS);
        let (key_name, pubkey) = parse_kms_uri(&uri).unwrap();
        assert_eq!(key_name, KEY_NAME);
        assert_eq!(pubkey, RFC8410_ADDRESS);
    }

    #[test]
    fn parse_kms_uri_rejects_missing_pubkey() {
        let err = parse_kms_uri(&format!("kms://{}", KEY_NAME)).unwrap_err();
        assert!(err.to_string().contains("pubkey="), "got: {err}");
    }

    #[test]
    fn parse_kms_uri_rejects_wrong_query_key() {
        let err = parse_kms_uri(&format!("kms://{}?address=abc", KEY_NAME)).unwrap_err();
        assert!(err.to_string().contains("pubkey="), "got: {err}");
    }

    #[test]
    fn parse_kms_uri_rejects_partial_resource_name() {
        let err = parse_kms_uri(&format!(
            "kms://projects/p/locations/l/keyRings/r/cryptoKeys/k?pubkey={}",
            RFC8410_ADDRESS
        ))
        .unwrap_err();
        assert!(err.to_string().contains("cryptoKeyVersions"), "got: {err}");
    }

    #[test]
    fn parse_kms_uri_rejects_bad_pubkey() {
        let err = parse_kms_uri(&format!("kms://{}?pubkey=not-base58!", KEY_NAME)).unwrap_err();
        assert!(err.to_string().contains("Invalid pubkey"), "got: {err}");
    }

    #[test]
    fn address_from_pem_rfc8410_vector() {
        let address = address_from_pem(RFC8410_PEM).unwrap();
        assert_eq!(address.to_string(), RFC8410_ADDRESS);
    }

    #[test]
    fn address_from_pem_rejects_non_ed25519() {
        // An EC P-256 SPKI (91 bytes) — must be rejected, not silently
        // truncated to its last 32 bytes.
        let p256_pem = "-----BEGIN PUBLIC KEY-----\n\
                        MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE6ZZ2b5nWiuM/aWEnPzVCf9ZAsL1E\n\
                        rTLNBb2/2vLdC0JYEB27Yg6E2P7VHzKQ35K0BiTAdCyfIPBGm8Fs82NB0g==\n\
                        -----END PUBLIC KEY-----\n";
        let err = address_from_pem(p256_pem).unwrap_err();
        assert!(err.to_string().contains("Ed25519"), "got: {err}");
    }

    #[test]
    fn address_from_pem_rejects_garbage() {
        assert!(address_from_pem("not a pem at all").is_err());
    }

    /// Build a KMS client against a wiremock endpoint with anonymous
    /// credentials — fully hermetic (no network, no ADC, no env vars).
    async fn mock_client(server_uri: &str) -> KeyManagementService {
        let credentials = google_cloud_auth::credentials::anonymous::Builder::new().build();
        KeyManagementService::builder()
            .with_endpoint(server_uri)
            .with_credentials(credentials)
            .build()
            .await
            .expect("failed to build mock KMS client")
    }

    /// Build a `KmsSigner` directly (bypassing the preflight) for tests that
    /// exercise signing.
    fn mock_signer(server_uri: &str, pubkey_b58: &str) -> KmsSigner {
        let inner = runtime()
            .block_on(async {
                let client = mock_client(server_uri).await;
                GcpKmsSigner::with_client(client, KEY_NAME.to_string(), pubkey_b58.to_string())
            })
            .expect("failed to build mock signer");
        KmsSigner { inner }
    }

    /// A PEM in the shape `getPublicKey` returns for an Ed25519 key.
    fn pem_for(pubkey: &Pubkey) -> String {
        let mut der = ED25519_SPKI_PREFIX.to_vec();
        der.extend_from_slice(&pubkey.to_bytes());
        format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            BASE64.encode(der)
        )
    }

    /// Run the preflight (`KmsSigner::with_client`) against a mocked
    /// `getPublicKey` response, declaring `declared` as the URI pubkey.
    fn preflight(response: ResponseTemplate, declared: &str) -> eyre::Result<KmsSigner> {
        runtime().block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(format!("/v1/{}/publicKey", KEY_NAME)))
                .respond_with(response)
                .mount(&server)
                .await;
            let client = mock_client(&server.uri()).await;
            KmsSigner::with_client(client, KEY_NAME.to_string(), declared.to_string()).await
        })
    }

    fn expect_err(result: eyre::Result<KmsSigner>) -> eyre::Report {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(e) => e,
        }
    }

    #[test]
    fn preflight_accepts_matching_key() {
        let pubkey = Keypair::new().pubkey();
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": KEY_NAME,
            "algorithm": "EC_SIGN_ED25519",
            "pem": pem_for(&pubkey)
        }));
        let signer = preflight(response, &pubkey.to_string()).unwrap();
        assert_eq!(signer.pubkey(), pubkey);
    }

    #[test]
    fn preflight_rejects_wrong_algorithm() {
        let pubkey = Keypair::new().pubkey();
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": KEY_NAME,
            "algorithm": "EC_SIGN_P256_SHA256",
            "pem": pem_for(&pubkey)
        }));
        let err = expect_err(preflight(response, &pubkey.to_string()));
        assert!(err.to_string().contains("EC_SIGN_ED25519"), "got: {err}");
    }

    /// A wrong `?pubkey=` must fail at preflight, naming the key's actual
    /// address — not surface later as an opaque signing error.
    #[test]
    fn preflight_rejects_mismatched_pubkey() {
        let actual = Keypair::new().pubkey();
        let declared = Keypair::new().pubkey();
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": KEY_NAME,
            "algorithm": "EC_SIGN_ED25519",
            "pem": pem_for(&actual)
        }));
        let err = expect_err(preflight(response, &declared.to_string()));
        assert!(err.to_string().contains(&actual.to_string()), "got: {err}");
    }

    /// API errors must surface with their real detail (status/message), not
    /// be collapsed into a guess list.
    #[test]
    fn preflight_surfaces_api_error_detail() {
        let response = ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": {
                "code": 403,
                "message": "Permission 'cloudkms.cryptoKeyVersions.viewPublicKey' denied",
                "status": "PERMISSION_DENIED"
            }
        }));
        let err = expect_err(preflight(response, &Keypair::new().pubkey().to_string()));
        let msg = err.to_string();
        assert!(msg.contains("Cannot access KMS key"), "got: {msg}");
        assert!(
            msg.contains("PERMISSION_DENIED") || msg.contains("denied"),
            "got: {msg}"
        );
    }

    /// End-to-end through the sync adapter: `try_sign_message` drives the
    /// async client on the module's runtime and returns a verified signature.
    #[test]
    fn adapter_signs_via_mock_kms() {
        // A real local keypair produces the mocked KMS response, so the
        // crate's verify-against-expected-pubkey check passes.
        let keypair = Keypair::new();
        let message = b"spherenet kms adapter test";
        let signature = keypair.sign_message(message);

        let server = runtime().block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(format!("/v1/{}:asymmetricSign", KEY_NAME)))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "signature": BASE64.encode(signature.as_ref()),
                    "verified_data_crc32c": true
                })))
                .expect(1)
                .mount(&server)
                .await;
            server
        });

        let signer = mock_signer(&server.uri(), &keypair.pubkey().to_string());
        assert_eq!(signer.pubkey(), keypair.pubkey());
        let got = signer.try_sign_message(message).unwrap();
        assert_eq!(got, signature);
    }

    /// The crate verifies returned signatures against the expected address:
    /// a signature from a different key must fail closed, surfaced as a
    /// `SignerError` through the adapter.
    #[test]
    fn adapter_rejects_signature_from_wrong_key() {
        let keypair = Keypair::new();
        let wrong_key = Keypair::new();
        let message = b"spherenet kms adapter test";
        let wrong_signature = wrong_key.sign_message(message);

        let server = runtime().block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(format!("/v1/{}:asymmetricSign", KEY_NAME)))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "signature": BASE64.encode(wrong_signature.as_ref()),
                    "verified_data_crc32c": true
                })))
                .mount(&server)
                .await;
            server
        });

        let signer = mock_signer(&server.uri(), &keypair.pubkey().to_string());
        assert!(signer.try_sign_message(message).is_err());
    }
}
