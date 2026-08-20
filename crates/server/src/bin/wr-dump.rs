//! `wr-dump`: connect to a wayland-remote server and dump frames.
//!
//! Connects over QUIC, performs the Hello/Welcome handshake, then accepts
//! the per-frame unidirectional streams, decompresses each frame, and writes
//! the raw BGRA bytes to stdout. Frame count/size diagnostics go to stderr
//! so stdout stays a clean byte stream (pipe it to a raw BGRA consumer or
//! split it into files at frame boundaries).
//!
//! The server certificate is either pinned with `--fingerprint <hex>`
//! (trust-on-first-use) or accepted unconditionally with `--insecure`
//! (testing only).

use std::io::Cursor;
use std::io::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use sha2::Digest;
use wayland_remote_protocol::{
    Compression, DecodeError, FRAME_HEADER_SIZE, FrameHeader, Message, decode_frame_header,
    decode_message, decompress, encode_message,
};
use wayland_remote_server::net::cert::ALPN_PROTOCOL;
use wayland_remote_server::rendering::FrameBuffer;

const USAGE: &str = "usage: wr-dump --addr ip:port (--fingerprint hex | --insecure)";

/// Signature schemes a custom verifier can claim to handle.
fn verify_schemes() -> Vec<rustls::SignatureScheme> {
    vec![
        rustls::SignatureScheme::RSA_PSS_SHA256,
        rustls::SignatureScheme::RSA_PKCS1_SHA256,
        rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
    ]
}

/// TOFU verifier: accepts the server only when the SHA-256 of the end-entity
/// certificate DER matches the pinned fingerprint.
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
        verify_schemes()
    }
}

/// `--insecure` verifier: accepts any server certificate. Testing only.
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
        verify_schemes()
    }
}

/// Parsed command line options.
struct Options {
    addr: SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
}

fn fail(message: &str) -> ! {
    eprintln!("wr-dump: {message}");
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn parse_args() -> Options {
    let mut addr: Option<SocketAddr> = None;
    let mut fingerprint: Option<[u8; 32]> = None;
    let mut insecure = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => {
                let value = args
                    .next()
                    .unwrap_or_else(|| fail("missing value for --addr"));
                addr = Some(
                    value
                        .parse::<SocketAddr>()
                        .unwrap_or_else(|_| fail(&format!("invalid --addr: {value}"))),
                );
            }
            "--fingerprint" => {
                let value = args
                    .next()
                    .unwrap_or_else(|| fail("missing value for --fingerprint"));
                let bytes: Vec<u8> = value
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(2)
                    .map(|pair| {
                        let mut hex = String::new();
                        hex.reserve(2);
                        hex.extend(pair);
                        u8::from_str_radix(&hex, 16)
                            .unwrap_or_else(|_| fail(&format!("invalid hex fingerprint: {value}")))
                    })
                    .collect();
                fingerprint =
                    Some(bytes.try_into().unwrap_or_else(|_| {
                        fail(&format!("fingerprint must be 32 bytes: {value}"))
                    }));
            }
            "--insecure" => insecure = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => fail(&format!("unknown argument: {other}")),
        }
    }
    let Some(addr) = addr else {
        fail("--addr is required");
    };
    Options {
        addr,
        fingerprint,
        insecure,
    }
}

/// Build the client's rustls config with the given certificate verifier.
fn client_config(verifier: Arc<dyn ServerCertVerifier>) -> anyhow::Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN_PROTOCOL.to_vec()];
    Ok(config)
}

/// Read one length-prefixed control message from the QUIC stream.
async fn read_message(recv: &mut quinn::RecvStream) -> anyhow::Result<Message> {
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match decode_message(&mut Cursor::new(&pending)) {
            Ok(msg) => return Ok(msg),
            Err(DecodeError::UnexpectedEof) => {
                // Incomplete message; pull more bytes.
            }
            Err(err) => anyhow::bail!("malformed control message: {err}"),
        }
        let mut buf = [0u8; 4096];
        match recv.read(&mut buf).await? {
            Some(n) => pending.extend_from_slice(&buf[..n]),
            None => anyhow::bail!("control stream closed before a complete message"),
        }
    }
}

/// Read one frame stream: 54-byte header plus the compressed payload.
async fn read_frame(
    recv: &mut quinn::RecvStream,
    header_out: &mut FrameHeader,
) -> anyhow::Result<FrameBuffer> {
    let mut header_bytes = [0u8; FRAME_HEADER_SIZE];
    recv.read_exact(&mut header_bytes).await?;
    *header_out = decode_frame_header(&mut Cursor::new(&header_bytes))?;
    let size = header_out.compressed_size as usize;
    if size > 64 * 1024 * 1024 {
        anyhow::bail!("frame payload too large: {size} bytes");
    }
    let mut payload = vec![0u8; size];
    recv.read_exact(&mut payload).await?;
    let data = match Compression::from_u8(header_out.compression)? {
        Compression::Lz4 => {
            let expected = (header_out.stride as u64 * header_out.height as u64) as usize;
            decompress(&payload, expected)?
        }
        Compression::None => payload,
    };
    Ok(FrameBuffer {
        data,
        width: header_out.width,
        height: header_out.height,
        stride: header_out.stride,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = parse_args();

    // Install the aws-lc-rs crypto provider once per process; a second
    // install is a no-op error that we ignore.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let verifier: Arc<dyn ServerCertVerifier> = match (opts.fingerprint, opts.insecure) {
        (Some(fingerprint), _) => Arc::new(FingerprintVerifier {
            expected: fingerprint,
        }),
        (None, true) => Arc::new(InsecureVerifier),
        (None, false) => fail("--fingerprint <hex> or --insecure is required"),
    };

    let client_config = client_config(verifier)?;
    let quic_config = quinn::crypto::rustls::QuicClientConfig::try_from(client_config)?;
    let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic_config)));

    let conn = endpoint.connect(opts.addr, "localhost")?.await?;
    eprintln!("wr-dump: connected to {}", opts.addr);

    // Handshake: Hello -> Welcome.
    let (mut ctrl_send, mut ctrl_recv) = conn.open_bi().await?;
    let mut hello = Vec::new();
    encode_message(
        &Message::Hello {
            version: 1,
            client_name: "wr-dump".to_owned(),
        },
        &mut hello,
    )?;
    ctrl_send.write_all(&hello).await?;
    match read_message(&mut ctrl_recv).await? {
        Message::Welcome {
            version,
            width,
            height,
        } => {
            eprintln!("wr-dump: server protocol v{version}, {width}x{height}");
        }
        other => anyhow::bail!("expected Welcome, got {other:?}"),
    }

    // Frames: one unidirectional stream each, in arrival order.
    let mut stdout = std::io::stdout().lock();
    let mut count: u64 = 0;
    let mut total: u64 = 0;
    let mut header = FrameHeader {
        magic: 0,
        frame_id: 0,
        window_id: 0,
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        compression: 0,
        _reserved: 0,
        timestamp_ns: 0,
        compressed_size: 0,
    };
    loop {
        let mut recv = match conn.accept_uni().await {
            Ok(recv) => recv,
            Err(err) => {
                eprintln!("wr-dump: connection closed: {err}");
                break;
            }
        };
        let frame = read_frame(&mut recv, &mut header).await?;
        count += 1;
        total += frame.data.len() as u64;
        stdout.write_all(&frame.data)?;
        stdout.flush()?;
        eprintln!(
            "wr-dump: frame {} (id {}, {}x{}, {} bytes, compression {})",
            count,
            header.frame_id,
            header.width,
            header.height,
            frame.data.len(),
            header.compression
        );
    }
    eprintln!("wr-dump: done: {count} frame(s), {total} bytes total");
    Ok(())
}
