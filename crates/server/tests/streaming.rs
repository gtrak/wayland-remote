//! QUIC streaming integration tests (plan 001 issue 05).
//!
//! Each test spawns a compositor server with the QUIC frame server enabled on
//! a free loopback port, connects a quinn client (insecure certificate
//! verifier), and exercises the wire protocol end to end: the Hello/Welcome
//! handshake, Ping/Pong echo, frame streaming from committed Wayland
//! surfaces, protocol-version rejection, and monotonic frame delivery.
//!
//! Tests drive their async code with an explicit multi-threaded tokio runtime
//! (`Runtime::new()`) instead of `#[tokio::test]`: the compositor thread uses
//! blocking I/O, and a single-threaded test runtime would conflict with it.

mod common;

use std::io::Cursor;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{PATTERN, TestClient, argb_to_bgra};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use wayland_remote_protocol::{
    Compression, FRAME_HEADER_SIZE, FrameHeader, Message, decode_frame_header, decompress,
    encode_message,
};
use wayland_remote_server::net::ERROR_VERSION_MISMATCH;
use wayland_remote_server::net::cert::{ALPN_PROTOCOL, ServerCert};
use wayland_remote_server::net::session::MessageReader;
use wayland_remote_server::run;
use wayland_remote_server::state::Config;

/// Unique socket name per server instance within this process.
static SOCKET_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The compositor binds its socket under `$XDG_RUNTIME_DIR`; the test process
/// cannot safely modify the environment (set_var is unsafe in edition 2024),
/// so the variable must be present in the test environment.
fn runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            panic!("XDG_RUNTIME_DIR must be set: compositor sockets are bound there")
        })
}

/// Load or generate the self-signed certificate once per test process.
///
/// The server does the same on its own thread; performing it here, serialized
/// by an `OnceLock`, keeps parallel tests from racing on the cert/key files.
fn ensure_cert() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        ServerCert::load_or_generate().expect("certificate should load or generate");
    });
}

/// Find a free UDP port on loopback, releasing the probe socket immediately.
fn free_port() -> u16 {
    let probe = UdpSocket::bind("127.0.0.1:0").expect("probe UDP socket should bind");
    let port = probe
        .local_addr()
        .expect("probe socket has a local address")
        .port();
    drop(probe);
    port
}

/// Spawn a server thread with QUIC streaming on a free loopback port.
/// Returns the QUIC listen address, the Wayland socket path, the status
/// receiver, the shutdown flag, and the thread handle.
fn spawn_streaming_server() -> (
    SocketAddr,
    PathBuf,
    mpsc::Receiver<usize>,
    Arc<AtomicBool>,
    thread::JoinHandle<anyhow::Result<()>>,
) {
    ensure_cert();
    let ip: std::net::IpAddr = "127.0.0.1"
        .parse()
        .expect("static loopback literal is a valid IP address");
    let listen = SocketAddr::new(ip, free_port());
    let socket_name = format!(
        "wayland-remote-stream-test-{}",
        SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let (status_tx, status_rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = Config {
        socket_name: Some(socket_name.clone()),
        listen: Some(listen),
        compression: Compression::Lz4,
        ..Config::default()
    };
    let shutdown_flag = shutdown.clone();
    let handle = thread::spawn(move || run(config, shutdown_flag, Some(status_tx), None));
    let socket_path = runtime_dir().join(&socket_name);

    // The socket file is created synchronously at startup; wait for it.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !socket_path.exists() {
        assert!(
            Instant::now() < deadline,
            "server socket did not appear at {socket_path:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }

    (listen, socket_path, status_rx, shutdown, handle)
}

/// Poll the status channel until the reported surface count equals
/// `expected`, failing after a 5 s deadline.
fn wait_for_count(rx: &mpsc::Receiver<usize>, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Ok(count) = rx.recv_timeout(Duration::from_millis(500)) {
        if count == expected {
            return;
        }
    }
    panic!("surface count did not reach {expected} before {deadline:?}");
}

/// Signal shutdown and join the server thread, asserting a clean exit.
fn stop_server(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<anyhow::Result<()>>) {
    shutdown.store(true, Ordering::SeqCst);
    let result = handle.join().expect("server thread should not panic");
    result.expect("server should exit cleanly");
}

/// Signature schemes a custom verifier can claim to handle.
fn verify_schemes() -> Vec<rustls::SignatureScheme> {
    vec![
        rustls::SignatureScheme::RSA_PSS_SHA256,
        rustls::SignatureScheme::RSA_PKCS1_SHA256,
        rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
    ]
}

/// Accepts any server certificate. Testing only.
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

/// A connected viewer: the QUIC connection plus the endpoint that keeps it
/// alive (dropping a quinn endpoint closes all of its connections).
struct Viewer {
    /// The QUIC connection to the frame server.
    pub conn: quinn::Connection,
    /// The endpoint holding the connection open.
    #[allow(dead_code)] // exists solely to keep `conn` alive
    endpoint: quinn::Endpoint,
}

/// Build a fresh quinn client endpoint and connect once to `addr`.
async fn connect_once(addr: SocketAddr) -> anyhow::Result<Viewer> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN_PROTOCOL.to_vec()];
    let quic_config = quinn::crypto::rustls::QuicClientConfig::try_from(config)?;
    let mut endpoint = quinn::Endpoint::client("127.0.0.1:0".parse()?)?;
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic_config)));
    let conn = endpoint.connect(addr, "localhost")?.await?;
    Ok(Viewer { conn, endpoint })
}

/// Connect a viewer to `addr`, retrying while the server's QUIC endpoint is
/// still binding (it starts asynchronously inside the server thread).
fn connect_viewer(runtime: &tokio::runtime::Runtime, addr: SocketAddr) -> Viewer {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last_err: String;
    loop {
        let attempt = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), connect_once(addr)).await
        });
        match attempt {
            Ok(Ok(viewer)) => return viewer,
            Ok(Err(err)) => last_err = format!("{err}"),
            Err(_) => last_err = "connect attempt timed out".to_owned(),
        }
        if Instant::now() >= deadline {
            panic!("QUIC connect to {addr} failed after retries: {last_err}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// The viewer's control stream: send side, receive side, and the framed
/// reader.
///
/// [`MessageReader`] is essential rather than cosmetic: a single QUIC read
/// can deliver several pipelined messages at once (loopback coalesces
/// freely), and only a reader that carries leftover bytes across calls
/// avoids dropping everything after the first message in the buffer.
struct ControlChannel {
    /// The client -> server half of the control stream.
    send: quinn::SendStream,
    /// The server -> client half of the control stream.
    recv: quinn::RecvStream,
    /// Framed reader preserving bytes that spilled past the last message.
    reader: MessageReader,
}

impl ControlChannel {
    /// Read the next framed control message.
    async fn next_message(&mut self) -> anyhow::Result<Message> {
        self.reader.next(&mut self.recv).await
    }
}

/// Read one frame stream: the 54-byte header plus the compressed payload.
/// Returns the decompressed BGRA pixels.
async fn read_frame(
    recv: &mut quinn::RecvStream,
    header_out: &mut FrameHeader,
) -> anyhow::Result<Vec<u8>> {
    let mut header_bytes = [0u8; FRAME_HEADER_SIZE];
    recv.read_exact(&mut header_bytes).await?;
    *header_out = decode_frame_header(&mut Cursor::new(&header_bytes))?;
    let size = header_out.compressed_size as usize;
    let mut payload = vec![0u8; size];
    recv.read_exact(&mut payload).await?;
    let pixels = match Compression::from_u8(header_out.compression)? {
        Compression::Lz4 => {
            let expected = (header_out.stride as u64 * header_out.height as u64) as usize;
            decompress(&payload, expected)?
        }
        Compression::None => payload,
    };
    Ok(pixels)
}

/// An all-zero [`FrameHeader`], to be overwritten by [`read_frame`].
fn blank_header() -> FrameHeader {
    FrameHeader {
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
    }
}

/// Open the control stream and perform the Hello -> Welcome handshake,
/// asserting the server advertises protocol v1 with the config's geometry.
///
/// Callers that expect frames must keep the returned channel alive: the
/// server closes the whole connection when the control stream ends.
async fn handshake(conn: &quinn::Connection) -> anyhow::Result<ControlChannel> {
    let (send, recv) = conn.open_bi().await?;
    let mut channel = ControlChannel {
        send,
        recv,
        reader: MessageReader::new(),
    };
    let mut hello = Vec::new();
    encode_message(
        &Message::Hello {
            version: 1,
            client_name: "streaming-test".to_owned(),
        },
        &mut hello,
    )?;
    channel.send.write_all(&hello).await?;
    match channel.next_message().await? {
        Message::Welcome {
            version,
            width,
            height,
        } => {
            assert_eq!(version, 1, "server should advertise protocol version 1");
            let default = Config::default();
            assert_eq!(
                width, default.width,
                "Welcome should advertise the output width"
            );
            assert_eq!(
                height, default.height,
                "Welcome should advertise the output height"
            );
        }
        other => anyhow::bail!("expected Welcome, got {other:?}"),
    }
    Ok(channel)
}

#[test]
fn handshake_and_ping() {
    // @lat: [[tests#Streaming#Handshake and ping]]
    let (listen, _socket_path, _status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let viewer = connect_viewer(&runtime, listen);

    let mut ctrl = runtime
        .block_on(handshake(&viewer.conn))
        .expect("handshake should succeed");

    runtime.block_on(async {
        let mut ping = Vec::new();
        encode_message(
            &Message::Ping {
                timestamp_ns: 12345,
            },
            &mut ping,
        )
        .expect("Ping should encode");
        ctrl.send.write_all(&ping).await.expect("Ping should write");

        // The server interleaves its own periodic Pings (the first fires
        // immediately); read until the Pong echoing our timestamp arrives.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "Pong for our Ping never arrived");
            match tokio::time::timeout(Duration::from_secs(2), ctrl.next_message()).await {
                Ok(Ok(Message::Pong { timestamp_ns })) => {
                    assert_eq!(timestamp_ns, 12345, "Pong must echo the Ping timestamp");
                    return;
                }
                Ok(Ok(Message::Ping { .. })) => {
                    // Server keepalive ping; ignored by the viewer.
                }
                Ok(Ok(other)) => panic!("unexpected control message: {other:?}"),
                Ok(Err(err)) => panic!("control stream read failed: {err}"),
                Err(_) => {
                    // Per-read timeout; keep waiting until the deadline.
                }
            }
        }
    });

    drop(ctrl);
    drop(viewer);
    stop_server(&shutdown, handle);
}

#[test]
fn frame_roundtrip() {
    // @lat: [[tests#Streaming#Frame roundtrip]]
    let (listen, socket_path, status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let viewer = connect_viewer(&runtime, listen);

    // Keep the control streams open for the whole test: the server closes the
    // connection when the control stream ends.
    let ctrl = runtime
        .block_on(handshake(&viewer.conn))
        .expect("handshake should succeed");

    // Commit a 64x64 surface; the server auto-renders and streams frames.
    let client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a surface");
    wait_for_count(&status_rx, 1);

    runtime.block_on(async {
        // Each frame arrives on its own unidirectional stream.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let mut recv = match tokio::time::timeout(
                Duration::from_secs(5),
                viewer.conn.accept_uni(),
            )
            .await
            {
                Ok(Ok(recv)) => recv,
                Ok(Err(err)) => panic!("frame stream unavailable: {err:?}"),
                Err(_) => {
                    assert!(
                        Instant::now() < deadline,
                        "no frame stream arrived within 10 s"
                    );
                    continue;
                }
            };
            let mut header = blank_header();
            let pixels = read_frame(&mut recv, &mut header)
                .await
                .expect("frame should read and decompress");
            assert!(
                header.width > 0 && header.height > 0,
                "frame must be non-empty"
            );
            let pixel = [pixels[0], pixels[1], pixels[2], pixels[3]];
            assert_eq!(
                pixel,
                argb_to_bgra(PATTERN),
                "top-left pixel must match the committed surface pattern"
            );
            return;
        }
    });

    drop(client);
    drop(ctrl);
    drop(viewer);
    stop_server(&shutdown, handle);
}

#[test]
fn version_mismatch_rejected() {
    // @lat: [[tests#Streaming#Version mismatch rejected]]
    let (listen, _socket_path, _status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let viewer = connect_viewer(&runtime, listen);

    runtime.block_on(async {
        let (mut ctrl_send, _ctrl_recv) = viewer
            .conn
            .open_bi()
            .await
            .expect("control stream should open");
        let mut hello = Vec::new();
        encode_message(
            &Message::Hello {
                version: 2,
                client_name: "test".to_owned(),
            },
            &mut hello,
        )
        .expect("Hello should encode");
        ctrl_send
            .write_all(&hello)
            .await
            .expect("Hello should write");

        let err = viewer.conn.closed().await;
        let quinn::ConnectionError::ApplicationClosed(close) = err else {
            panic!("expected an application close, got {err:?}");
        };
        assert_eq!(
            close.error_code.into_inner(),
            u64::from(ERROR_VERSION_MISMATCH),
            "close code must be the version-mismatch application error"
        );
    });

    drop(viewer);
    stop_server(&shutdown, handle);
}

#[test]
fn frame_coalescing() {
    // @lat: [[tests#Streaming#Frame coalescing]]
    let (listen, socket_path, status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let viewer = connect_viewer(&runtime, listen);

    // Keep the control streams open: the server closes the connection when
    // the control stream ends.
    let ctrl = runtime
        .block_on(handshake(&viewer.conn))
        .expect("handshake should succeed");

    let client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a surface");
    wait_for_count(&status_rx, 1);

    // The server auto-renders on every compositor tick, so frames stream
    // without further prompting. Collect them for ~500 ms and check that
    // delivered frame ids increase monotonically (coalescing can skip ids
    // but never regress them).
    let frame_ids = runtime.block_on(async {
        let mut ids: Vec<u64> = Vec::new();
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(500) {
            match tokio::time::timeout(Duration::from_millis(250), viewer.conn.accept_uni()).await {
                Ok(Ok(mut recv)) => {
                    let mut header = blank_header();
                    read_frame(&mut recv, &mut header)
                        .await
                        .expect("frame should read and decompress");
                    ids.push(header.frame_id);
                }
                Ok(Err(err)) => panic!("frame stream unavailable: {err:?}"),
                Err(_) => {
                    // No stream yet; keep collecting.
                }
            }
        }
        ids
    });

    assert!(
        !frame_ids.is_empty(),
        "expected at least one frame within the collection window"
    );
    for pair in frame_ids.windows(2) {
        assert!(
            pair[1] > pair[0],
            "frame ids must increase monotonically: {frame_ids:?}"
        );
    }

    drop(client);
    drop(ctrl);
    drop(viewer);
    stop_server(&shutdown, handle);
}
