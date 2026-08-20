//! QUIC client session for the viewer.

use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use sha2::Digest;
use wayland_remote_protocol::{
    Compression, DecodeError, FRAME_HEADER_SIZE, InputEvent, Message, decode_frame_header,
    decode_message, decompress, encode_message,
};

use crate::framebuf::FrameBuffer;

/// Insecure verifier for testing — accepts any certificate.
#[derive(Debug)]
struct InsecureVerifier;

impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        ]
    }
}

/// TOFU verifier: accepts the server only when the SHA-256 of the
/// end-entity certificate DER matches the pinned fingerprint.
#[derive(Debug)]
struct FingerprintVerifier {
    expected: [u8; 32],
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let actual: [u8; 32] = sha2::Sha256::digest(end_entity.as_ref()).into();
        if actual == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "certificate fingerprint mismatch".into(),
            ))
        }
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        ]
    }
}

const ALPN: &[u8] = b"wayland-remote/1";

/// An active QUIC connection to the server with a control stream open.
pub struct ViewerSession {
    conn: quinn::Connection,
    ctrl_send: quinn::SendStream,
    ctrl_recv: quinn::RecvStream,
    pub width: u32,
    pub height: u32,
}

impl ViewerSession {
    /// Connect to the server, perform the Hello/Welcome handshake.
    pub async fn connect(
        addr: SocketAddr,
        fingerprint: Option<[u8; 32]>,
        insecure: bool,
    ) -> anyhow::Result<Self> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let verifier: Arc<dyn ServerCertVerifier> = match (fingerprint, insecure) {
            (Some(fp), _) => Arc::new(FingerprintVerifier { expected: fp }),
            (None, true) => Arc::new(InsecureVerifier),
            (None, false) => anyhow::bail!("--fingerprint <hex> or --insecure is required"),
        };

        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        config.alpn_protocols = vec![ALPN.to_vec()];

        let quic_config = quinn::crypto::rustls::QuicClientConfig::try_from(config)?;
        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic_config)));

        let conn = endpoint.connect(addr, "localhost")?.await?;

        let (mut ctrl_send, mut ctrl_recv) = conn.open_bi().await?;

        // Send Hello
        let mut hello = Vec::new();
        encode_message(
            &Message::Hello {
                version: 1,
                client_name: "wayland-remote-viewer".to_owned(),
            },
            &mut hello,
        )?;
        ctrl_send.write_all(&hello).await?;

        // Read Welcome
        let welcome = read_message(&mut ctrl_recv).await?;
        let (width, height) = match welcome {
            Message::Welcome {
                version,
                width,
                height,
            } => {
                if version != 1 {
                    anyhow::bail!("protocol version mismatch: server v{version}");
                }
                (width, height)
            }
            other => anyhow::bail!("expected Welcome, got {other:?}"),
        };

        Ok(Self {
            conn,
            ctrl_send,
            ctrl_recv,
            width,
            height,
        })
    }

    /// Receive the next frame from a new unidirectional stream.
    pub async fn next_frame(&self) -> anyhow::Result<FrameBuffer> {
        let mut recv = self.conn.accept_uni().await?;
        let mut header_bytes = [0u8; FRAME_HEADER_SIZE];
        recv.read_exact(&mut header_bytes).await?;
        let header = decode_frame_header(&mut Cursor::new(&header_bytes))?;

        let size = header.compressed_size as usize;
        let mut payload = vec![0u8; size];
        recv.read_exact(&mut payload).await?;

        let data = match Compression::from_u8(header.compression)? {
            Compression::Lz4 => {
                let expected = (header.stride as u64 * header.height as u64) as usize;
                decompress(&payload, expected)?
            }
            Compression::None => payload,
        };

        Ok(FrameBuffer {
            data,
            width: header.width,
            height: header.height,
            stride: header.stride,
            frame_id: header.frame_id,
            timestamp_ns: header.timestamp_ns,
        })
    }

    /// Send an input event to the server.
    pub async fn send_input(&mut self, window_id: u64, event: InputEvent) -> anyhow::Result<()> {
        let mut buf = Vec::new();
        encode_message(&Message::Input { window_id, event }, &mut buf)?;
        self.ctrl_send.write_all(&buf).await?;
        Ok(())
    }

    /// Send a Ping and wait for the Pong, returning the RTT in nanoseconds.
    pub async fn ping(&mut self) -> anyhow::Result<u64> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos() as u64;
        let mut buf = Vec::new();
        encode_message(&Message::Ping { timestamp_ns: ts }, &mut buf)?;
        self.ctrl_send.write_all(&buf).await?;

        // Read until we get a Pong with our timestamp
        loop {
            match read_message(&mut self.ctrl_recv).await? {
                Message::Pong { timestamp_ns } if timestamp_ns == ts => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos() as u64;
                    return Ok(now - ts);
                }
                _ => continue,
            }
        }
    }

    /// Close the connection.
    pub fn close(&self) {
        self.conn.close(u32::MAX.into(), b"viewer closing");
    }
}

/// Read one length-prefixed control message from a QUIC stream.
async fn read_message(recv: &mut quinn::RecvStream) -> anyhow::Result<Message> {
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match decode_message(&mut Cursor::new(&pending)) {
            Ok(msg) => return Ok(msg),
            Err(DecodeError::UnexpectedEof) => {}
            Err(err) => anyhow::bail!("malformed control message: {err}"),
        }
        let mut buf = [0u8; 4096];
        match recv.read(&mut buf).await? {
            Some(n) => pending.extend_from_slice(&buf[..n]),
            None => anyhow::bail!("control stream closed before a complete message"),
        }
    }
}
