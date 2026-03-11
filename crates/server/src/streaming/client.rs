//! Per-client connection handler for TCP frame streaming
//!
//! Handles individual viewer connections with:
//! - Bounded mpsc channel for backpressure
//! - Frame streaming via encode_frame()
//! - Graceful disconnect handling
//! - Client registration/deregistration

use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};

use super::protocol::{encode_frame, FrameHeader};
use super::{StreamingState, ClientHandle};

/// Handle a single client connection
///
/// This function manages the lifecycle of a viewer connection:
/// 1. Register client on connect
/// 2. Set up bounded channel for backpressure
/// 3. Stream frames to client
/// 4. Unregister client on disconnect
///
/// # Arguments
/// * `socket` - The TCP socket for the client
/// * `addr` - Client address
/// * `state` - Shared streaming state
///
/// # Returns
/// Ok(()) on normal disconnect, Err on error
pub async fn handle_client(
    socket: TcpStream,
    addr: SocketAddr,
    state: Arc<RwLock<StreamingState>>,
) -> anyhow::Result<()> {
    info!("Handling client connection from {}", addr);

    // Register client
    let handle = ClientHandle::new(addr);
    state.write().await.register_client(addr, handle).await;

    debug!("Client {} registered ({} clients total)", 
           addr, state.read().await.client_count().await);

    // Create bounded channel for backpressure (32 frame buffer)
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);

    // Split the socket into read and write halves
    let (mut read_half, mut write_half) = socket.into_split();

    // Spawn frame sender task - this owns the write half of the socket
    let send_handle = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(frame_data) => {
                    match write_half.write_all(&frame_data).await {
                        Ok(_) => {
                            debug!("Sent frame to {}", addr);
                        }
                        Err(e) => {
                            warn!("Failed to write frame to {}: {}", addr, e);
                            break;
                        }
                    }
                }
                None => {
                    debug!("Channel closed for {}, stopping sender", addr);
                    break;
                }
            }
        }
        // Write half is dropped here
    });
    // Spawn frame streaming task - continuously reads frames and sends to client
    let state_clone = state.clone();
    let tx_clone = tx.clone();
    let stream_handle = tokio::spawn(async move {
        if let Err(e) = stream_frames(tx_clone, state_clone, addr).await {
            debug!("Frame streaming error for {}: {}", addr, e);
        }
    });

    // Read loop - handle incoming viewer requests using read half
    // For now, we just wait for the client to disconnect
    // Future: could implement request/response protocol here
    let mut buf = [0u8; 1024];
    match read_half.read(&mut buf).await {
        Ok(_) => {
            debug!("Client {} sent data", addr);
        }
        Err(e) => {
            debug!("Client {} disconnect: {}", addr, e);
        }
    }

    // Drop sender to signal frame sender task to stop
    drop(tx);

    // Wait for sender and streamer tasks to complete
    let _ = send_handle.await;
    let _ = stream_handle.await;

    // Unregister client on disconnect
    state.write().await.unregister_client(addr).await;
    
    info!("Client {} disconnected", addr);

    Ok(())
}

/// Stream frames to a client
///
/// Continuously reads frames from the streaming state and sends them
/// to the client via the bounded channel. Implements backpressure by
/// using try_send() - if the channel is full, frames are dropped.
///
/// # Arguments
/// * `tx` - Sender for the bounded channel
/// * `state` - Shared streaming state
/// * `addr` - Client address (for logging)
///
/// # Returns
/// Ok(()) when channel is closed, Err on error
pub async fn stream_frames(
    tx: mpsc::Sender<Vec<u8>>,
    state: Arc<RwLock<StreamingState>>,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    use std::time::{SystemTime, UNIX_EPOCH};

    loop {
        // Get all surfaces
        let surfaces = state.read().await.get_all_surfaces().await;
        
        for (window_id, frame_data) in surfaces.iter() {
            // Create frame header
            let timestamp_us = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;

            let header = FrameHeader::new(*window_id, frame_data.width, frame_data.height, timestamp_us);

            // Encode frame
            let encoded = encode_frame(&header, &frame_data.rgba);

            // Try to send with backpressure
            // If channel is full, log warning and drop frame
            if tx.try_send(encoded.to_vec()).is_err() {
                warn!("Client {} backpressure - dropping frame {}", addr, window_id);
            }
        }

        // Small delay to prevent tight loop
        // Future: use proper frame rate control
        tokio::time::sleep(tokio::time::Duration::from_millis(33)).await; // ~30fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_handle_client_basic() {
        // Create a listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Create streaming state
        let state = Arc::new(RwLock::new(StreamingState::new()));

        // Accept connection in background
        let accept_handle = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            (socket, addr)
        });

        // Give accept time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Connect client
        let client_socket = tokio::net::TcpStream::connect(addr).await.unwrap();

        // Get accepted socket
        let (server_socket, client_addr) = accept_handle.await.unwrap();

        // Handle the client (will disconnect immediately since we close client_socket)
        let handle_result = handle_client(server_socket, client_addr, state).await;

        // Should succeed (normal disconnect)
        assert!(handle_result.is_ok());

        // Client should be unregistered
        assert_eq!(state.read().await.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_client_registration() {
        let state = Arc::new(RwLock::new(StreamingState::new()));
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        // Manually test registration
        let handle = ClientHandle::new(addr);
        state.write().await.register_client(addr, handle).await;
        
        assert_eq!(state.read().await.client_count().await, 1);
        
        state.write().await.unregister_client(addr).await;
        assert_eq!(state.read().await.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_bounded_channel_backpressure() {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(2);

        // Fill the channel
        assert!(tx.try_send(vec![1]).is_ok());
        assert!(tx.try_send(vec![2]).is_ok());
        
        // Should fail with backpressure
        assert!(tx.try_send(vec![3]).is_err());

        // Drain one
        let _ = rx.recv().await.unwrap();
        
        // Should succeed now
        assert!(tx.try_send(vec![3]).is_ok());
    }
}
