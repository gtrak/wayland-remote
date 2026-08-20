---
name: quinn-quic-api
description: >-
  Exact quinn 0.11 / rcgen 0.14 / rustls 0.23 API contract for QUIC transport
  in wayland-remote. Read this before writing QUIC networking code.
---

# Quinn 0.11 + rcgen 0.14 + rustls 0.23 API Contract

Verified against docs.rs and the quinn/rcgen/rustls crate APIs.

## Server setup

### Self-signed cert (rcgen 0.14)

```rust
use rcgen::generate_simple_self_signed;

// Generate a self-signed cert
let cert = generate_simple_self_signed(vec!["localhost".into()])?;
let cert_pem = cert.cert.pem();
let key_pem = cert.key_pair.serialize_pem();

// For rustls, we need DER format:
let cert_der = cert.cert.der();  // CertificateDer
let key_der = cert.key_pair.serialize_der();  // PrivateKeyDer
```

### Persisting certs

Save cert.pem + key.pem to `$XDG_DATA_HOME/wayland-remote/` (or `~/.local/share/`).
On startup: try to load; if missing, generate + save. Print the SHA-256 of the
cert DER as a hex fingerprint for TOFU.

```rust
use sha2::{Sha256, Digest};
let fingerprint = hex::encode(Sha256::digest(cert.cert.der().as_ref()));
// NOTE: sha2 is already a smithay transitive dep, but may need adding directly.
// hex encoding: use `format!("{:x}", Sha256::digest(cert_der))` if hex crate unavailable.
```

### rustls ServerConfig with aws-lc-rs

```rust
use rustls;

// Install aws-lc-rs as the default crypto provider (do this once at startup):
rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .expect("Failed to install aws-lc-rs provider");

// Build server config:
let mut server_config = rustls::ServerConfig::builder_with_provider(
    rustls::crypto::aws_lc_rs::default_provider()
)
.with_safe_default_protocol_versions()?
.with_no_client_auth()
.with_single_cert(
    vec![cert_der.clone()],  // Vec<CertificateDer>
    key_der.clone(),          // PrivateKeyDer
)?;

// Set ALPN:
server_config.alpn_protocols = vec![b"wayland-remote/1".to_vec()];
```

### Quinn server endpoint

```rust
use quinn;

// Convert rustls config to quinn:
let quic_server_config = quinn::crypto::rustls::QuicServerConfig::try_from(server_config)?;
let quinn_server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_server_config));

// Create endpoint:
let endpoint = quinn::Endpoint::server(
    quinn_server_config,
    "0.0.0.0:9000".parse()?,
)?;

// Accept loop:
while let Some(incoming) = endpoint.accept().await {
    let conn = incoming.await?;  // Connection
    tokio::spawn(handle_connection(conn));
}
```

## Client setup (for wr-dump / viewer)

### TOFU fingerprint verifier

```rust
use rustls::client::danger::ServerCertVerifier;
use rustls::client::danger::ServerCertVerified;

struct FingerprintVerifier {
    expected_fingerprint: [u8; 32],  // SHA-256 of cert DER
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let hash = sha2::Sha256::digest(end_entity.as_ref());
        if hash.as_slice() == self.expected_fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("fingerprint mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // We don't care about signature verification — we trust the fingerprint
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![rustls::SignatureScheme::RSA_PSS_SHA256, rustls::SignatureScheme::ECDSA_NISTP256_SHA256]
    }
}
```

NOTE: The exact trait methods may differ slightly in rustls 0.23.43. Check the
compiler errors and adapt. The key is: implement `ServerCertVerifier`, compare
the cert's SHA-256 against the expected fingerprint, and use `.assertion()` for
the valid types (they're zero-sized marker types).

### Client config + endpoint

```rust
// For --insecure (no fingerprint check): use a verifier that always returns Ok.
// For --fingerprint <hex>: parse hex, create FingerprintVerifier.

let verifier = Arc::new(FingerprintVerifier { expected_fingerprint });
let mut client_config = rustls::ClientConfig::builder_with_provider(
    rustls::crypto::aws_lc_rs::default_provider()
)
.with_safe_default_protocol_versions()?
.dangerous()
.with_custom_certificate_verifier(verifier)?
.with_no_client_auth();

client_config.alpn_protocols = vec![b"wayland-remote/1".to_vec()];

let quic_client_config = quinn::crypto::rustls::QuicClientConfig::try_from(client_config)?;
let quinn_client_config = quinn::ClientConfig::new(Arc::new(quic_client_config));

let endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;
endpoint.set_default_client_config(quinn_client_config);

// Connect:
let conn = endpoint.connect("127.0.0.1:9000".parse()?, "localhost")?.await?;
```

## Streams

### Control stream (bidirectional)

```rust
// Server accepts:
let (mut send, mut recv) = conn.accept_bi().await?;

// Client opens:
let (mut send, mut recv) = conn.open_bi().await?;

// Write:
send.write_all(&bytes).await?;
send.write_all(&more_bytes).await?;

// Read:
let mut buf = [0u8; 1024];
let n = recv.read(&mut buf).await?;  // Option<usize> — None means stream ended

// Finish sending (half-close):
send.finish()?;

// Stop receiving (sends STOP_SENDING to peer):
recv.stop(0u64)?;  // error code 0
```

### Frame streams (unidirectional)

```rust
// Server opens (one per frame):
let mut send = conn.open_uni().await?;
send.write_all(&frame_header_bytes).await?;
send.write_all(&pixel_data).await?;
send.finish()?;

// Client accepts:
let mut recv = conn.accept_uni().await?;  // RecvStream

// Skip-stale: when a newer frame stream arrives, stop the old one:
old_recv.stop(0u64)?;  // sends STOP_SENDING
```

## Connection close

```rust
conn.close(0u32::MAX, b"shutdown");
```

## Protocol flow

1. QUIC handshake (TLS 1.3 with self-signed cert + ALPN "wayland-remote/1")
2. Client opens bi stream → sends Hello → reads Welcome
3. Server: accept bi → read Hello → send Welcome
4. Then: server opens uni stream per frame; client reads from control stream for input
5. Client: ping every 500ms → server echoes pong
6. On disconnect: server cleans up session, keeps accepting new connections

## Cargo.toml deps

Already in workspace: quinn, rustls, rcgen. The server crate already has them.
For `sha2`: it's a smithay transitive dep but may need to be added directly:
```toml
sha2 = "0.10"
```
For `hex`: avoid the hex crate; use `format!("{:x}", ...)` on a byte slice:
```rust
let hash = sha2::Sha256::digest(der_bytes);
let hex = hash.iter().map(|b| format!("{:02x}", b)).collect::<String>();
```

## calloop ↔ tokio bridge

The compositor thread runs calloop (NOT async). The network runs tokio. Bridge:

```rust
// Compositor → Network (frames out):
// calloop thread pushes FrameBuffer via tokio::sync::mpsc::UnboundedSender
// Use unbounded_send (non-async, never blocks on unbounded channel)
let (frame_tx, frame_rx) = tokio::sync::mpsc::unbounded_channel::<FrameBuffer>();
// In calloop thread:
frame_tx.send(frame)?;  // unbounded_send, never blocks

// Network → Compositor (input in):
// tokio task sends InputEvent via calloop channel
let (input_tx, input_rx) = calloop::channel::sync_channel::<InputEvent>(100);
// In calloop loop:
handle.insert_source(input_rx, |event, _, state| {
    // handle input event
})?;
// In tokio task:
input_tx.send(event)?;  // sync, may block if full
```

## wr-dump binary

`crates/server/src/bin/wr-dump.rs`:
- Args: `--addr <ip:port>`, `--fingerprint <hex>` (or `--insecure`)
- Connects via quinn client
- Opens control stream, sends Hello, reads Welcome
- Accepts uni streams (frames), decodes FrameHeader + decompresses (lz4)
- Writes raw BGRA to stdout
- Prints frame count + size to stderr

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // parse args
    // connect quinn
    // handshake
    // loop: accept_uni → read FrameHeader → read payload → decompress if lz4 → write to stdout
}
```
