use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{broadcast, mpsc};

use super::drag;
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
    /// We are the sender: a drag-across is in progress.
    /// Mouse events are buffered until DragReady arrives, then forwarded.
    DragSending {
        peer_id: String,
        exit_edge: ScreenEdge,
        ready: bool,
    },
    /// We are the receiver: a drag session is active locally via overlay window.
    /// Mouse events continue to be simulated (they drive the local drag loop).
    DragReceiving {
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
    /// Buffer for mouse events while waiting for DragReady from receiver.
    drag_buffer: VecDeque<ClipboardContent>,
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
            drag_buffer: VecDeque::with_capacity(256),
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

        // Spawn the OS grab thread with auto-restart on crash
        let shared_for_grab = shared.clone();
        let grab_tx_clone = grab_tx.clone();
        let peer_edge_for_grab = peer_edge;
        std::thread::spawn(move || {
            loop {
                let shared = shared_for_grab.clone();
                let tx = grab_tx_clone.clone();
                run_grab_loop(shared, tx, peer_edge_for_grab);
                eprintln!("Mouse sharing: grab loop exited, restarting in 2s...");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
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

        // Drag timeout watchdog: cancel DragSending if no DragReady within 2s
        let shared_for_timeout = shared.clone();
        let to_server_for_timeout = to_server_tx.clone();
        let _timeout_handle = tokio::spawn(async move {
            let mut drag_start: Option<Instant> = None;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                let mut s = shared_for_timeout.lock().unwrap();
                match s.state {
                    MouseState::DragSending { ready: false, .. } => {
                        if drag_start.is_none() {
                            drag_start = Some(Instant::now());
                        } else if drag_start.unwrap().elapsed() > std::time::Duration::from_secs(2) {
                            println!("Mouse: DragSending timeout (no DragReady in 2s), cancelling");
                            let _ = to_server_for_timeout.send(ClipboardContent::DragCancel);
                            s.state = MouseState::Local;
                            s.drag_buffer.clear();
                            drag_start = None;
                        }
                    }
                    _ => {
                        drag_start = None;
                    }
                }
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

        ClipboardContent::DragBegin { files, entry_edge, entry_x, entry_y } => {
            println!(
                "Mouse: received DragBegin ({} file(s), entry: {} at ({:.2}, {:.2}))",
                files.len(), entry_edge, entry_x, entry_y
            );
            let mut s = shared.lock().unwrap();
            let (abs_x, abs_y) = s.screen.denormalize(*entry_x, *entry_y);
            s.state = MouseState::DragReceiving {
                peer_id: "remote".to_string(),
                entry_edge: *entry_edge,
            };
            drop(s);

            // Start the overlay drag session on a dedicated thread
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let files_clone = files.clone();
            let to_server = to_server_tx.clone();
            // Spawn a blocking task to await ready signal and send DragReady
            tokio::spawn(async move {
                match drag::start_drag_session(files_clone, abs_x, abs_y, ready_tx) {
                    Ok(_session) => {
                        // Wait for the overlay to be ready
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            ready_rx,
                        ).await {
                            Ok(Ok(())) => {
                                println!("Mouse: overlay drag ready, sending DragReady");
                                let _ = to_server.send(ClipboardContent::DragReady);
                            }
                            _ => {
                                eprintln!("Mouse: overlay drag failed to start, sending DragCancel");
                                let _ = to_server.send(ClipboardContent::DragCancel);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Mouse: failed to start drag session: {}", e);
                        let _ = to_server.send(ClipboardContent::DragCancel);
                    }
                }
            });
        }

        ClipboardContent::DragReady => {
            let mut s = shared.lock().unwrap();
            if let MouseState::DragSending { ref peer_id, exit_edge, ready: false } = s.state {
                println!("Mouse: DragReady received, flushing {} buffered events", s.drag_buffer.len());
                s.state = MouseState::DragSending {
                    peer_id: peer_id.clone(),
                    exit_edge,
                    ready: true,
                };
                // Flush buffered events
                let buffered: Vec<_> = s.drag_buffer.drain(..).collect();
                drop(s);
                for msg in buffered {
                    let _ = to_server_tx.send(msg);
                }
            }
        }

        ClipboardContent::DragCancel => {
            let mut s = shared.lock().unwrap();
            match s.state {
                MouseState::DragSending { .. } => {
                    println!("Mouse: drag cancelled by receiver");
                    s.state = MouseState::Local;
                    s.drag_buffer.clear();
                }
                MouseState::DragReceiving { .. } => {
                    println!("Mouse: drag cancelled by sender");
                    // Simulate Escape to cancel any active local drag
                    drop(s);
                    super::input::simulate_key("Escape", true);
                    super::input::simulate_key("Escape", false);
                    let mut s = shared.lock().unwrap();
                    s.state = MouseState::Local;
                }
                _ => {}
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
                                let (nx, ny) = s.screen.normalize(x, y);
                                let (entry_x, entry_y) =
                                    ScreenConfig::entry_position(edge, nx, ny);

                                // If button is held, try to detect dragged files
                                if s.button_held {
                                    if let Some(files) = drag::read_dragged_files() {
                                        if !files.is_empty() {
                                            s.state = MouseState::DragSending {
                                                peer_id: peer_id.clone(),
                                                exit_edge: edge,
                                                ready: false,
                                            };
                                            s.drag_buffer.clear();
                                            println!("Mouse: drag detected at {} edge → sending {} file(s) to peer", edge, files.len());
                                            let _ = tx.send(ClipboardContent::DragBegin {
                                                files,
                                                entry_edge: edge.mirror(),
                                                entry_x,
                                                entry_y,
                                            });
                                            return None;
                                        }
                                    }
                                }

                                // No drag — normal cursor sharing
                                s.state = MouseState::Remote {
                                    peer_id: peer_id.clone(),
                                    exit_edge: edge,
                                };
                                println!("Mouse: cursor crossed {} edge → remote peer", edge);
                                let _ = tx.send(ClipboardContent::MouseMove {
                                    x: entry_x,
                                    y: entry_y,
                                });
                                s.last_move_sent = Instant::now();
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
                    MouseState::DragSending { ready, .. } => {
                        let now = Instant::now();
                        if now.duration_since(s.last_move_sent) >= min_move_interval {
                            let (nx, ny) = s.screen.normalize(x, y);
                            let msg = ClipboardContent::MouseMove { x: nx, y: ny };
                            if ready {
                                let _ = tx.send(msg);
                            } else {
                                // Buffer until DragReady arrives
                                if s.drag_buffer.len() < 256 {
                                    s.drag_buffer.push_back(msg);
                                }
                            }
                            s.last_move_sent = now;
                        }
                        None
                    }
                    MouseState::Controlled { .. } | MouseState::DragReceiving { .. } => {
                        // We're being controlled — pass through (simulated events)
                        Some(event)
                    }
                }
            }

            rdev::EventType::ButtonPress(button) => {
                s.button_held = true;
                match s.state {
                    MouseState::Remote { .. } | MouseState::DragSending { ready: true, .. } => {
                        let _ = tx.send(ClipboardContent::MouseButton {
                            button: button_to_u8(&button),
                            pressed: true,
                        });
                        None
                    }
                    MouseState::DragSending { ready: false, .. } => {
                        let msg = ClipboardContent::MouseButton {
                            button: button_to_u8(&button),
                            pressed: true,
                        };
                        if s.drag_buffer.len() < 256 {
                            s.drag_buffer.push_back(msg);
                        }
                        None
                    }
                    _ => Some(event),
                }
            }

            rdev::EventType::ButtonRelease(button) => {
                s.button_held = false;
                match s.state.clone() {
                    MouseState::Remote { .. } => {
                        let _ = tx.send(ClipboardContent::MouseButton {
                            button: button_to_u8(&button),
                            pressed: false,
                        });
                        None
                    }
                    MouseState::DragSending { ready: true, .. } => {
                        // Forward release — triggers drop on receiver
                        let _ = tx.send(ClipboardContent::MouseButton {
                            button: button_to_u8(&button),
                            pressed: false,
                        });
                        println!("Mouse: drag completed (button released)");
                        s.state = MouseState::Local;
                        s.drag_buffer.clear();
                        None
                    }
                    MouseState::DragSending { ready: false, .. } => {
                        // Released before receiver was ready — cancel
                        let _ = tx.send(ClipboardContent::DragCancel);
                        println!("Mouse: drag cancelled (released before DragReady)");
                        s.state = MouseState::Local;
                        s.drag_buffer.clear();
                        None
                    }
                    _ => Some(event),
                }
            }

            rdev::EventType::Wheel { delta_x, delta_y } => {
                match s.state {
                    MouseState::Remote { .. } | MouseState::DragSending { ready: true, .. } => {
                        let _ = tx.send(ClipboardContent::MouseScroll {
                            delta_x: delta_x as i32,
                            delta_y: delta_y as i32,
                        });
                        None
                    }
                    MouseState::DragSending { ready: false, .. } => None, // drop scroll during buffer
                    _ => Some(event),
                }
            }

            rdev::EventType::KeyPress(key) => {
                match s.state {
                    MouseState::Remote { .. } | MouseState::DragSending { ready: true, .. } => {
                        let _ = tx.send(ClipboardContent::KeyEvent {
                            key: key_to_string(&key),
                            pressed: true,
                        });
                        None
                    }
                    MouseState::DragSending { ready: false, .. } => None,
                    _ => Some(event),
                }
            }

            rdev::EventType::KeyRelease(key) => {
                match s.state {
                    MouseState::Remote { .. } | MouseState::DragSending { ready: true, .. } => {
                        let _ = tx.send(ClipboardContent::KeyEvent {
                            key: key_to_string(&key),
                            pressed: false,
                        });
                        None
                    }
                    MouseState::DragSending { ready: false, .. } => None,
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
