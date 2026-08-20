//! No-GUI scripted client mode: connects, runs a scripted input sequence,
//! captures frames, detects pixel changes, writes PNGs, prints a JSON summary.
//!
//! This is the QUIC client that a cross-machine driver invokes to exercise the
//! server headlessly (no display required). Pure async — compiles on Windows
//! and Linux.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use wayland_remote_protocol::{ButtonState, InputEvent, Message, WindowEventKind};

use crate::framebuf::FrameBuffer;
use crate::session::ViewerSession;
use crate::window_manager::ViewerWindowManager;

/// A single scripted action for the drive sequence.
#[derive(Debug, Clone)]
pub enum DriveAction {
    Click { x: f64, y: f64, button: u32 },
    KeyPress { scancode: u16 },
    Wait { ms: u64 },
}

/// Configuration for a drive run.
pub struct DriveConfig {
    pub addr: SocketAddr,
    pub fingerprint: Option<[u8; 32]>,
    pub insecure: bool,
    pub actions: Vec<DriveAction>,
    pub max_frames: usize,
    pub out_dir: PathBuf,
}

/// A pixel change detected while capturing frames after an action.
#[derive(Debug, Clone, Copy)]
struct PixelChange {
    frame: u64,
    ms: u64,
}

/// Connect, run the scripted sequence, and print a JSON summary to stdout.
pub fn run_drive(config: DriveConfig) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let out_dir = config.out_dir.clone();
        std::fs::create_dir_all(&out_dir)?;

        let mut session =
            ViewerSession::connect(config.addr, config.fingerprint, config.insecure).await?;
        tracing::info!(
            addr = %config.addr,
            "drive: connected, {}x{}",
            session.width,
            session.height
        );

        let mut windows = ViewerWindowManager::new();
        let rtt_ns = session.ping().await?;

        // Phase 1 — wait for a window to be created.
        let mut window_id: Option<u64> = None;
        let phase_start = Instant::now();
        while window_id.is_none() {
            if phase_start.elapsed() > Duration::from_secs(5) {
                anyhow::bail!("no window created within timeout");
            }
            if let Some(Message::WindowEvent {
                window_id: wid,
                event,
            }) = session.try_read_control().await
            {
                windows.handle_event(wid, &event);
                if matches!(event, WindowEventKind::Created { .. }) {
                    window_id = Some(wid);
                }
            }
        }
        let window_id = window_id.unwrap_or(0);
        tracing::info!(window_id, "drive: window created");

        // Phase 2 — baseline frame.
        let baseline = match tokio::time::timeout(Duration::from_secs(2), session.next_frame()).await
        {
            Ok(Ok(frame)) => {
                write_frame_png(&frame, &out_dir.join("frame_0.png"))?;
                Some(frame.data)
            }
            _ => anyhow::bail!("no baseline frame within timeout"),
        };

        let mut total_frames = 1u64;
        let mut frame_seq = 0u64;
        let mut remaining = config.max_frames;
        let mut pixels_changed_at: Option<PixelChange> = None;
        let action_start = Instant::now();

        // Phase 3 — execute the scripted actions, capturing frames as they go.
        for action in &config.actions {
            match action {
                DriveAction::Click { x, y, button } => {
                    session
                        .send_input(window_id, InputEvent::PointerMove { x: *x, y: *y })
                        .await?;
                    session
                        .send_input(
                            window_id,
                            InputEvent::PointerButton {
                                button: *button,
                                state: ButtonState::Pressed,
                            },
                        )
                        .await?;
                    session
                        .send_input(
                            window_id,
                            InputEvent::PointerButton {
                                button: *button,
                                state: ButtonState::Released,
                            },
                        )
                        .await?;
                }
                DriveAction::KeyPress { scancode } => {
                    session
                        .send_input(window_id, InputEvent::KeyDown { scancode: *scancode })
                        .await?;
                    session
                        .send_input(window_id, InputEvent::KeyUp { scancode: *scancode })
                        .await?;
                }
                DriveAction::Wait { ms } => {
                    tokio::time::sleep(Duration::from_millis(*ms)).await;
                }
            }

            // After each action, capture frames until a change is detected or the
            // frame budget is exhausted.
            while remaining > 0 && pixels_changed_at.is_none() {
                while let Some(msg) = session.try_read_control().await {
                    if let Message::WindowEvent {
                        window_id: wid,
                        event,
                    } = msg
                    {
                        windows.handle_event(wid, &event);
                    }
                }
                match tokio::time::timeout(Duration::from_secs(2), session.next_frame()).await {
                    Ok(Ok(frame)) => {
                        frame_seq += 1;
                        total_frames += 1;
                        remaining -= 1;
                        if let Some(b) = &baseline
                            && frame.data != *b
                        {
                            let ms = action_start.elapsed().as_millis() as u64;
                            pixels_changed_at = Some(PixelChange {
                                frame: frame_seq,
                                ms,
                            });
                            write_frame_png(
                                &frame,
                                &out_dir.join(format!("frame_{frame_seq}.png")),
                            )?;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "drive: frame error, stopping capture");
                        break;
                    }
                    Err(_) => {
                        tracing::debug!("drive: no frame within 2s");
                        break;
                    }
                }
            }
        }

        // Phase 4 — report.
        let elapsed_secs = action_start.elapsed().as_secs_f64();
        let fps = if elapsed_secs > 0.0 {
            total_frames as f64 / elapsed_secs
        } else {
            0.0
        };
        println!(
            "{{\"frames\":{},\"fps\":{:.1},\"rtt_ns\":{},\"pixels_changed_at\":{},\"window_id\":{}}}",
            total_frames,
            fps,
            rtt_ns,
            match &pixels_changed_at {
                Some(pc) => format!("{{\"frame\":{},\"ms\":{}}}", pc.frame, pc.ms),
                None => "null".to_string(),
            },
            window_id,
        );

        session.close();
        Ok(())
    })
}

/// Convert a BGRA frame to an RGBA PNG and save it at `path`.
///
/// Follows the server's [`FrameBuffer::write_png`] pattern (swap B and R
/// channels). Frames from the server are contiguous (`stride == width * 4`);
/// if a padded stride is ever seen, rows are copied one at a time.
fn write_frame_png(frame: &FrameBuffer, path: &Path) -> anyhow::Result<()> {
    let w = frame.width as usize;
    let h = frame.height as usize;
    let row_bytes = w * 4;
    let stride = frame.stride as usize;

    let mut rgba = Vec::with_capacity(row_bytes * h);
    if stride == row_bytes {
        for pixel in frame.data.chunks_exact(4) {
            rgba.push(pixel[2]);
            rgba.push(pixel[1]);
            rgba.push(pixel[0]);
            rgba.push(pixel[3]);
        }
    } else {
        for y in 0..h {
            let row_start = y * stride;
            let row = frame
                .data
                .get(row_start..row_start + row_bytes)
                .ok_or_else(|| anyhow::anyhow!("frame data shorter than stride * height"))?;
            for pixel in row.chunks_exact(4) {
                rgba.push(pixel[2]);
                rgba.push(pixel[1]);
                rgba.push(pixel[0]);
                rgba.push(pixel[3]);
            }
        }
    }

    let img = image::RgbaImage::from_raw(frame.width, frame.height, rgba)
        .ok_or_else(|| anyhow::anyhow!("invalid frame dimensions for PNG"))?;
    img.save(path)?;
    Ok(())
}
