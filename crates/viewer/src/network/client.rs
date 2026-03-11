//! Async TCP client for frame streaming
//!
//! Connects to the Linux server and receives frames via Tokio async runtime.
//! Frames are sent to the main thread via mpsc channel to avoid blocking UI.

use crate::network::{Frame, FrameHeader, NetworkError};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// TCP client that receives frames from the server
///
/// Runs in a dedicated Tokio runtime thread to avoid blocking the UI thread.
/// Decoded frames are sent via mpsc channel to the receiver.
pub struct TcpClient {
    /// Address of the server (host:port)
    address: String,
}

impl TcpClient {
    /// Create a new TCP client configured to connect to the given address
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
        }
    }

    /// Connect to the server and start receiving frames
    ///
    /// # Arguments
    /// * `rx` - Receiver end of the channel to receive frames
    ///
    /// # Returns
    /// Handle to the spawned task that can be used to await completion
    pub async fn connect(&self) -> Result<TcpStream, NetworkError> {
        info!(address = %self.address, "Connecting to server");
        
        let stream = TcpStream::connect(&self.address)
            .await
            .map_err(|e| NetworkError::Connection(format!("Failed to connect: {}", e)))?;
        
        info!(address = %self.address, "Connected successfully");
        Ok(stream)
    }

    /// Read a complete frame from the stream
    ///
    /// Reads the 20-byte header first, then reads the payload based on
    /// the dimensions specified in the header.
    ///
    /// # Arguments
    /// * `stream` - TCP stream to read from
    ///
    /// # Errors
    /// Returns NetworkError if connection is lost or protocol is invalid
    pub async fn read_frame(&self, stream: &mut TcpStream) -> Result<Frame, NetworkError> {
        read_frame_from_stream(stream).await
    }

    /// Start receiving frames in a background task
    ///
    /// Spawns a task that continuously reads frames from the stream and
    /// sends them via the channel. Returns the receiver end of the channel.
    ///
    /// # Arguments
    /// * `stream` - Connected TCP stream
    /// * `buffer_size` - Channel buffer size (number of frames to buffer)
    ///
    /// # Returns
    /// Receiver end of the mpsc channel for receiving frames
    pub async fn start_receiving(
        stream: TcpStream,
        buffer_size: usize,
    ) -> mpsc::Receiver<Frame> {
        let (tx, rx) = mpsc::channel(buffer_size);
        
        tokio::spawn(async move {
            let mut stream = stream;
            
            loop {
                match read_frame_from_stream(&mut stream).await {
                    Ok(frame) => {
                        if tx.send(frame).await.is_err() {
                            warn!("Receiver dropped, stopping frame reception");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error reading frame: {}", e);
                        break;
                    }
                }
            }
        });

        rx
    }
}

/// Read a complete frame from a TCP stream
///
/// This is a standalone helper function that can be called from spawned tasks
/// without requiring a TcpClient instance.
async fn read_frame_from_stream(stream: &mut TcpStream) -> Result<Frame, NetworkError> {
    // Read 20-byte header
    let mut header_buf = [0u8; FrameHeader::SIZE];
    stream
        .read_exact(&mut header_buf)
        .await
        .map_err(|e| NetworkError::Protocol(format!("Failed to read header: {}", e)))?;

    // Decode header
    let header = FrameHeader::decode(&header_buf)?;
    debug!(
        window_id = header.window_id,
        width = header.width,
        height = header.height,
        timestamp = header.timestamp,
        "Received frame header"
    );

    // Calculate payload size
    let payload_size = header.payload_size();
    
    // Sanity check to prevent unreasonable allocations
    if payload_size > 100_000_000 {
        return Err(NetworkError::Protocol(format!(
            "Unreasonable payload size: {} bytes",
            payload_size
        )));
    }

    // Read payload
    let mut data = vec![0u8; payload_size];
    stream
        .read_exact(&mut data)
        .await
        .map_err(|e| NetworkError::Protocol(format!("Failed to read payload: {}", e)))?;

    debug!(size = payload_size, "Received frame payload");

    Ok(Frame::new(header, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_client_creation() {
        let client = TcpClient::new("127.0.0.1:9999");
        assert_eq!(client.address, "127.0.0.1:9999");
    }

    #[tokio::test]
    async fn test_connection_refused() {
        let client = TcpClient::new("127.0.0.1:1");
        let result = client.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_frame_from_mock_server() {
        // Start a mock server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn server task that sends a frame
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            
            // Create test frame: window_id=1, 10x10 RGBA
            let header = FrameHeader {
                window_id: 1,
                width: 10,
                height: 10,
                timestamp: 1234567890,
            };
            
            let header_bytes = header.encode();
            let payload = vec![0xFFu8; 10 * 10 * 4]; // Red pixels
            
            stream.write_all(&header_bytes).await.unwrap();
            stream.write_all(&payload).await.unwrap();
        });

        // Connect client
        let client = TcpClient::new(addr.to_string());
        let mut stream = client.connect().await.unwrap();
        
        // Read frame
        let frame = client.read_frame(&mut stream).await.unwrap();
        
        assert_eq!(frame.header.window_id, 1);
        assert_eq!(frame.header.width, 10);
        assert_eq!(frame.header.height, 10);
        assert_eq!(frame.header.timestamp, 1234567890);
        assert_eq!(frame.data.len(), 10 * 10 * 4);
        assert!(frame.is_valid());
    }

    #[tokio::test]
    async fn test_read_frame_incomplete_header() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Server sends only 15 bytes (incomplete header)
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(&[0u8; 15]).await.unwrap();
        });

        let client = TcpClient::new(addr.to_string());
        let mut stream = client.connect().await.unwrap();
        
        let result = client.read_frame(&mut stream).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_frame_incomplete_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Server sends complete header but incomplete payload
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            
            let header = FrameHeader {
                window_id: 1,
                width: 100,
                height: 100,
                timestamp: 0,
            };
            
            stream.write_all(&header.encode()).await.unwrap();
            // Send only half the payload
            stream.write_all(&vec![0u8; 50 * 100 * 4]).await.unwrap();
        });

        let client = TcpClient::new(addr.to_string());
        let mut stream = client.connect().await.unwrap();
        
        let result = client.read_frame(&mut stream).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_client_address_parsing() {
        let client = TcpClient::new("192.168.1.100:8080");
        assert_eq!(client.address, "192.168.1.100:8080");
        
        let client2 = TcpClient::new("localhost:9000");
        assert_eq!(client2.address, "localhost:9000");
    }
}
