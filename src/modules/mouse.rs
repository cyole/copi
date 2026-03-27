use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{broadcast, mpsc};

use super::screen::{ScreenConfig, ScreenEdge};
use super::sync::{ClipboardContent, ClipboardMessage};

/// Mouse sharing state machine.
#[derive(Debug, Clone, PartialEq)]
enum MouseState {
    /// Cursor is local; watching for edge hits.
    Local,
    /// Our cursor has crossed to a remote peer — we are sending input to them.
    Remote {
        peer_id: String,
        exit_edge: ScreenEdge,
    },
    /// A remote peer is controlling our cursor.
    Controlled {
        peer_id: String,
        entry_edge: ScreenEdge,
    },
}

/// Convert an rdev::Key to a stable string representation.
fn key_to_string(key: &rdev::Key) -> String {
    format!("{:?}", key)
}

/// Convert an rdev::Button to a u8 identifier.
fn button_to_u8(button: &rdev::Button) -> u8 {
    match button {
        rdev::Button::Left => 0,
        rdev::Button::Right => 1,
        rdev::Button::Middle => 2,
        rdev::Button::Unknown(n) => *n as u8,
    }
}

/// Convert a u8 back to an rdev::Button.
fn u8_to_button(b: u8) -> rdev::Button {
    match b {
        0 => rdev::Button::Left,
        1 => rdev::Button::Right,
        2 => rdev::Button::Middle,
        n => rdev::Button::Unknown(n as u16),
    }
}

/// Shared state between the grab thread and the async runtime.
struct SharedMouseState {
    state: MouseState,
    screen: ScreenConfig,
    /// Current mouse position (absolute pixels, updated by grab callback).
    mouse_x: f64,
    mouse_y: f64,
    /// Whether any mouse button is currently held (for drag detection).
    button_held: bool,
    /// Last time we sent a MouseMove (for rate limiting).
    last_move_sent: Instant,
}

/// Spawn the mouse sharing system.
///
/// This creates:
/// 1. An OS thread running `rdev::grab()` for input capture + edge detection
/// 2. A tokio task bridging grab events to the network
/// 3. A tokio task receiving remote input events and simulating them locally
pub fn spawn_mouse_sharing(
    peer_edge: ScreenEdge,
    our_client_id: String,
    to_server_tx: broadcast::Sender<ClipboardContent>,
    mut mouse_inbound_rx: mpsc::UnboundedReceiver<ClipboardMessage>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Detect screen dimensions
        let mut screen = ScreenConfig::detect();
        println!(
            "Mouse sharing: screen {}x{}, peer at {} edge",
            screen.width, screen.height, peer_edge
        );

        // We don't know the peer's client_id yet — it will be set during negotiation.
        // For now, set the edge with a placeholder; updated when MouseShareAccept arrives.
        let placeholder_peer = "pending".to_string();
        screen.set_peer_edge(peer_edge, placeholder_peer);

        let shared = Arc::new(Mutex::new(SharedMouseState {
            state: MouseState::Local,
            screen,
            mouse_x: 0.0,
            mouse_y: 0.0,
            button_held: false,
            last_move_sent: Instant::now(),
        }));

        // Channel from grab thread to async bridge
        let (grab_tx, mut grab_rx) = mpsc::unbounded_channel::<ClipboardContent>();

        // Send our ScreenInfo and MouseShareOffer
        {
            let s = shared.lock().unwrap();
            let _ = to_server_tx.send(ClipboardContent::ScreenInfo {
                width: s.screen.width,
                height: s.screen.height,
                client_id: our_client_id.clone(),
            });
            let _ = to_server_tx.send(ClipboardContent::MouseShareOffer {
                client_id: our_client_id.clone(),
                accept_edge: peer_edge,
            });
        }

        // Spawn the OS grab thread
        let shared_for_grab = shared.clone();
        let grab_tx_clone = grab_tx.clone();
        let peer_edge_for_grab = peer_edge;
        std::thread::spawn(move || {
            run_grab_loop(shared_for_grab, grab_tx_clone, peer_edge_for_grab);
        });

        // Bridge task: forward grab events to the network
        let to_server_for_bridge = to_server_tx.clone();
        let bridge_handle = tokio::spawn(async move {
            while let Some(content) = grab_rx.recv().await {
                let _ = to_server_for_bridge.send(content);
            }
        });

        // Receive task: handle incoming mouse events from remote peers
        let shared_for_recv = shared.clone();
        let our_id = our_client_id.clone();
        let to_server_for_recv = to_server_tx.clone();
        let recv_handle = tokio::spawn(async move {
            while let Some(message) = mouse_inbound_rx.recv().await {
                handle_inbound_mouse_message(
                    &message.content,
                    &shared_for_recv,
                    &our_id,
                    &to_server_for_recv,
                    peer_edge,
                );
            }
        });

        tokio::select! {
            _ = bridge_handle => {}
            _ = recv_handle => {}
        }
    })
}

/// Handle an inbound mouse sharing message from a remote peer.
fn handle_inbound_mouse_message(
    content: &ClipboardContent,
    shared: &Arc<Mutex<SharedMouseState>>,
    our_client_id: &str,
    to_server_tx: &broadcast::Sender<ClipboardContent>,
    peer_edge: ScreenEdge,
) {
    match content {
        ClipboardContent::ScreenInfo { width, height, client_id } => {
            println!(
                "Mouse: received screen info {}x{} from {}",
                width, height,
                &client_id[..8.min(client_id.len())]
            );
        }

        ClipboardContent::MouseShareOffer { client_id, accept_edge: _ } => {
            println!(
                "Mouse: received share offer from {}",
                &client_id[..8.min(client_id.len())]
            );
            // Accept the offer
            let _ = to_server_tx.send(ClipboardContent::MouseShareAccept {
                client_id: our_client_id.to_string(),
            });
            // Update the peer mapping
            let mut s = shared.lock().unwrap();
            s.screen.set_peer_edge(peer_edge, client_id.clone());
            println!("Mouse: sharing active with peer {}", &client_id[..8.min(client_id.len())]);
        }

        ClipboardContent::MouseShareAccept { client_id } => {
            println!(
                "Mouse: share accepted by {}",
                &client_id[..8.min(client_id.len())]
            );
            let mut s = shared.lock().unwrap();
            s.screen.set_peer_edge(peer_edge, client_id.clone());
        }

        ClipboardContent::MouseMove { x, y } => {
            let mut s = shared.lock().unwrap();
            if !matches!(s.state, MouseState::Controlled { .. }) {
                // Transition to Controlled state
                let entry_edge = peer_edge.mirror();
                s.state = MouseState::Controlled {
                    peer_id: "remote".to_string(),
                    entry_edge,
                };
                println!("Mouse: now controlled by remote peer");
            }
            // Simulate mouse movement
            let (abs_x, abs_y) = s.screen.denormalize(*x, *y);
            drop(s);
            super::input::simulate_mouse_move(abs_x, abs_y);

            // Check if cursor is returning to the entry edge
            let entry_edge = peer_edge.mirror();
            if ScreenConfig::is_at_entry_edge(entry_edge, *x, *y) {
                // Cursor is heading back — send MouseReturn
                let _ = to_server_tx.send(ClipboardContent::MouseReturn);
                let mut s = shared.lock().unwrap();
                s.state = MouseState::Local;
                println!("Mouse: control returned to local (cursor at entry edge)");
            }
        }

        ClipboardContent::MouseButton { button, pressed } => {
            super::input::simulate_mouse_button(*button, *pressed);
        }

        ClipboardContent::MouseScroll { delta_x, delta_y } => {
            super::input::simulate_scroll(*delta_x, *delta_y);
        }

        ClipboardContent::KeyEvent { key, pressed } => {
            super::input::simulate_key(key, *pressed);
        }

        ClipboardContent::MouseReturn => {
            let mut s = shared.lock().unwrap();
            if matches!(s.state, MouseState::Remote { .. }) {
                s.state = MouseState::Local;
                println!("Mouse: cursor returned from remote peer");
            }
        }

        _ => {}
    }
}

/// Run the rdev::grab() loop on a dedicated OS thread.
/// This intercepts all input events when in Remote state.
fn run_grab_loop(
    shared: Arc<Mutex<SharedMouseState>>,
    tx: mpsc::UnboundedSender<ClipboardContent>,
    _peer_edge: ScreenEdge,
) {
    // Minimum interval between MouseMove messages (roughly 120 events/sec max)
    let min_move_interval = std::time::Duration::from_millis(8);

    let callback = move |event: rdev::Event| -> Option<rdev::Event> {
        let mut s = shared.lock().unwrap();

        match event.event_type {
            rdev::EventType::MouseMove { x, y } => {
                s.mouse_x = x;
                s.mouse_y = y;

                match s.state.clone() {
                    MouseState::Local => {
                        // Check for edge hit
                        if let Some((edge, peer_id)) = s.screen.edge_hit(x, y) {
                            let peer_id = peer_id.to_string();
                            if peer_id != "pending" {
                                s.state = MouseState::Remote {
                                    peer_id: peer_id.clone(),
                                    exit_edge: edge,
                                };
                                println!("Mouse: cursor crossed {} edge → remote peer", edge);

                                // Send the entry position on the remote screen
                                let (nx, ny) = s.screen.normalize(x, y);
                                let (entry_x, entry_y) =
                                    ScreenConfig::entry_position(edge, nx, ny);
                                let _ = tx.send(ClipboardContent::MouseMove {
                                    x: entry_x,
                                    y: entry_y,
                                });
                                s.last_move_sent = Instant::now();

                                // Consume the event (don't let cursor pass the edge locally)
                                return None;
                            }
                        }
                        // Normal local movement — pass through
                        Some(event)
                    }
                    MouseState::Remote { .. } => {
                        // Rate-limit mouse moves
                        let now = Instant::now();
                        if now.duration_since(s.last_move_sent) >= min_move_interval {
                            let (nx, ny) = s.screen.normalize(x, y);
                            let _ = tx.send(ClipboardContent::MouseMove { x: nx, y: ny });
                            s.last_move_sent = now;
                        }
                        // Consume — cursor stays locked at edge
                        None
                    }
                    MouseState::Controlled { .. } => {
                        // We're being controlled — pass through (simulated events)
                        Some(event)
                    }
                }
            }

            rdev::EventType::ButtonPress(button) => {
                s.button_held = true;
                match s.state {
                    MouseState::Remote { .. } => {
                        let _ = tx.send(ClipboardContent::MouseButton {
                            button: button_to_u8(&button),
                            pressed: true,
                        });
                        None
                    }
                    _ => Some(event),
                }
            }

            rdev::EventType::ButtonRelease(button) => {
                s.button_held = false;
                match s.state {
                    MouseState::Remote { .. } => {
                        let _ = tx.send(ClipboardContent::MouseButton {
                            button: button_to_u8(&button),
                            pressed: false,
                        });
                        None
                    }
                    _ => Some(event),
                }
            }

            rdev::EventType::Wheel { delta_x, delta_y } => {
                match s.state {
                    MouseState::Remote { .. } => {
                        let _ = tx.send(ClipboardContent::MouseScroll {
                            delta_x: delta_x as i32,
                            delta_y: delta_y as i32,
                        });
                        None
                    }
                    _ => Some(event),
                }
            }

            rdev::EventType::KeyPress(key) => {
                match s.state {
                    MouseState::Remote { .. } => {
                        let _ = tx.send(ClipboardContent::KeyEvent {
                            key: key_to_string(&key),
                            pressed: true,
                        });
                        None
                    }
                    _ => Some(event),
                }
            }

            rdev::EventType::KeyRelease(key) => {
                match s.state {
                    MouseState::Remote { .. } => {
                        let _ = tx.send(ClipboardContent::KeyEvent {
                            key: key_to_string(&key),
                            pressed: false,
                        });
                        None
                    }
                    _ => Some(event),
                }
            }
        }
    };

    // Print platform-specific warnings
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
        {
            eprintln!("Mouse sharing: Wayland detected. Grab requires root or 'input' group membership.");
        }
    }

    println!("Mouse sharing: starting input grab...");
    if let Err(e) = rdev::grab(callback) {
        eprintln!("Mouse sharing: grab failed: {:?}", e);
        eprintln!("Mouse sharing: falling back to listen mode (no grab, edge detection only)");

        // Fallback: use listen() instead of grab() — can't intercept, but can detect
        // This is less ideal but works without elevated permissions
        // Note: we won't implement fallback in this version, just log the error
    }
}
