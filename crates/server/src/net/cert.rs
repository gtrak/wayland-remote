//! Self-signed TLS certificate management for the QUIC endpoint.
//!
//! The server persists an rcgen-generated keypair as DER files under
//! `$XDG_DATA_HOME/wayland-remote/` (falling back to `~/.local/share/`) so a
//! stable fingerprint exists across restarts. The viewer pins that SHA-256
//! fingerprint of the certificate DER trust-on-first-use
//! ([[decisions#Decision Log#Crypto]]).

use std::path::{Path, PathBuf};

use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};

/// ALPN protocol identifier negotiated during the QUIC handshake.
pub const ALPN_PROTOCOL: &[u8] = b"wayland-remote/1";

/// Self-signed certificate, key, and trust-on-first-use fingerprint.
#[derive(Debug)]
pub struct ServerCert {
    /// The certificate in DER encoding.
    pub cert_der: CertificateDer<'static>,
    /// The private key in DER encoding.
    pub key_der: PrivateKeyDer<'static>,
    /// Hex-encoded SHA-256 fingerprint of the certificate DER.
    pub fingerprint: String,
}

impl ServerCert {
    /// Generate a fresh self-signed certificate in memory (no file I/O).
    pub fn generate() -> anyhow::Result<Self> {
        let params = generate_simple_self_signed(vec!["localhost".into()])?;
        let cert_der = params.cert.der().clone();
        let key_der = PrivateKeyDer::try_from(params.signing_key.serialize_der())
            .map_err(|err| anyhow::anyhow!("invalid generated key: {err}"))?;
        Ok(Self {
            fingerprint: fingerprint_str(cert_der.as_ref()),
            cert_der,
            key_der,
        })
    }

    /// Load the persisted keypair, or generate and persist a new one.
    ///
    /// Reads `cert.der` and `key.der` from the data directory; if either file
    /// is missing (or the key no longer parses), a new pair is generated and
    /// both files are rewritten.
    pub fn load_or_generate() -> anyhow::Result<Self> {
        let dir = data_dir()?;
        let cert_path = dir.join("cert.der");
        let key_path = dir.join("key.der");

        match (std::fs::read(&cert_path), std::fs::read(&key_path)) {
            (Ok(cert_bytes), Ok(key_bytes)) => {
                if let Ok(key_der) = PrivateKeyDer::try_from(key_bytes) {
                    let fingerprint = fingerprint_str(&cert_bytes);
                    return Ok(Self {
                        cert_der: CertificateDer::from(cert_bytes),
                        key_der,
                        fingerprint,
                    });
                }
                tracing::warn!("persisted key is invalid; regenerating certificate pair");
            }
            (Ok(_), Err(_)) | (Err(_), Ok(_)) => {
                tracing::warn!("certificate/key pair incomplete; regenerating");
            }
            (Err(_), Err(_)) => {}
        }

        let cert = Self::generate()?;
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&cert_path, cert.cert_der.as_ref())?;
        let key_bytes: Vec<u8> = match &cert.key_der {
            rustls::pki_types::PrivateKeyDer::Pkcs1(d) => d.secret_pkcs1_der().to_vec(),
            rustls::pki_types::PrivateKeyDer::Sec1(d) => d.secret_sec1_der().to_vec(),
            rustls::pki_types::PrivateKeyDer::Pkcs8(d) => d.secret_pkcs8_der().to_vec(),
            _ => cert.key_der.secret_der().to_vec(),
        };
        std::fs::write(&key_path, &key_bytes)?;
        Ok(cert)
    }
}

/// The directory the keypair is persisted under.
fn data_dir() -> anyhow::Result<PathBuf> {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| anyhow::anyhow!("neither XDG_DATA_HOME nor HOME is set"))?;
            Path::new(&home).join(".local/share")
        }
    };
    Ok(base.join("wayland-remote"))
}

/// SHA-256 of `der`, hex-encoded (64 lowercase chars).
#[must_use]
pub fn fingerprint_str(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_for_same_der() {
        let der = b"fake-certificate-bytes";
        assert_eq!(fingerprint_str(der), fingerprint_str(der));
        assert_eq!(fingerprint_str(der).len(), 64);
        assert_ne!(fingerprint_str(der), fingerprint_str(b"other-bytes"));
    }

    #[test]
    fn generate_yields_valid_pair() {
        let cert = ServerCert::generate().expect("generation should succeed");
        assert_eq!(cert.fingerprint, fingerprint_str(cert.cert_der.as_ref()));
        assert_eq!(cert.fingerprint.len(), 64);
    }
}
