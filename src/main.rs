mod modules;

use anyhow::Result;
use clap::{Parser, Subcommand};
use modules::clipboard::ClipboardMonitor;
use modules::files::FileMonitor;
use modules::sync::{ClipboardContent, ClipboardMessage, SyncClient, SyncServer};
use modules::tls;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::sync::{broadcast, mpsc};

#[derive(Parser)]
#[command(name = "copi")]
#[command(about = "A cross-platform clipboard synchronization tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Server {
        #[arg(short, long, default_value = "0.0.0.0:9527")]
        addr: SocketAddr,

        /// Relay-only mode: no clipboard access, only relay between clients (for headless servers)
        #[arg(short, long)]
        relay_only: bool,

        /// Authentication token (HMAC challenge-response). Clients must provide the same token.
        /// Can also be set via COPI_TOKEN environment variable.
        #[arg(short, long, env = "COPI_TOKEN")]
        token: Option<String>,

        /// Server secret — clients must provide this to connect at all.
        /// Can also be set via COPI_SECRET environment variable.
        #[arg(long, env = "COPI_SECRET")]
        secret: Option<String>,

        /// Path to TLS certificate file (PEM). Enables TLS when provided with --key.
        #[arg(long, env = "COPI_TLS_CERT")]
        cert: Option<String>,

        /// Path to TLS private key file (PEM). Enables TLS when provided with --cert.
        #[arg(long, env = "COPI_TLS_KEY")]
        key: Option<String>,

        /// Auto-generate a self-signed TLS certificate (for development/testing)
        #[arg(long)]
        tls_auto_cert: bool,

        /// Directory to sync files from/to. Files in this directory are synced across all peers.
        #[arg(long, env = "COPI_SYNC_DIR")]
        sync_dir: Option<PathBuf>,

        /// Maximum file size to sync in megabytes (default: 10 MB)
        #[arg(long, default_value = "10")]
        max_file_size: u64,
    },
    Client {
        /// Server address (hostname or IP, port defaults to 9527)
        #[arg(short, long)]
        server: String,

        #[arg(short, long, default_value = "0.0.0.0:9528")]
        listen: SocketAddr,

        /// Authentication token for connecting to a token-protected server.
        /// Can also be set via COPI_TOKEN environment variable.
        #[arg(short, long, env = "COPI_TOKEN")]
        token: Option<String>,

        /// Server secret for gate authentication.
        /// Can also be set via COPI_SECRET environment variable.
        #[arg(long, env = "COPI_SECRET")]
        secret: Option<String>,

        /// Enable TLS connection to server
        #[arg(long)]
        tls: bool,

        /// Path to CA certificate file (PEM) for verifying server's TLS certificate
        #[arg(long, env = "COPI_TLS_CA_CERT")]
        ca_cert: Option<String>,

        /// Skip TLS certificate verification (for self-signed certs)
        #[arg(long)]
        tls_skip_verify: bool,

        /// Directory to sync files from/to. Files in this directory are synced across all peers.
        #[arg(long, env = "COPI_SYNC_DIR")]
        sync_dir: Option<PathBuf>,

        /// Maximum file size to sync in megabytes (default: 10 MB)
        #[arg(long, default_value = "10")]
        max_file_size: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server {
            addr,
            relay_only,
            token,
            secret,
            cert,
            key,
            tls_auto_cert,
            sync_dir,
            max_file_size,
        } => {
            run_server(addr, relay_only, token, secret, cert, key, tls_auto_cert, sync_dir, max_file_size)
                .await?;
        }
        Commands::Client {
            server,
            listen,
            token,
            secret,
            tls,
            ca_cert,
            tls_skip_verify,
            sync_dir,
            max_file_size,
        } => {
            run_client(
                server,
                listen,
                token,
                secret,
                tls,
                ca_cert,
                tls_skip_verify,
                sync_dir,
                max_file_size,
            )
            .await?;
        }
    }

    Ok(())
}

fn log_content(prefix: &str, content: &ClipboardContent) {
    match content {
        ClipboardContent::Text(text) => {
            println!("{}: text ({} bytes)", prefix, text.len());
        }
        ClipboardContent::Image { width, height, .. } => {
            println!("{}: image ({}x{})", prefix, width, height);
        }
        ClipboardContent::Html { html, .. } => {
            println!("{}: html ({} bytes)", prefix, html.len());
        }
        ClipboardContent::File { path, size, .. } => {
            println!("{}: file \"{}\" ({} bytes)", prefix, path, size);
        }
        ClipboardContent::FileCopy { files } => {
            let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
            let total: u64 = files.iter().map(|f| f.size).sum();
            println!(
                "{}: {} file(s) [{}] ({} bytes)",
                prefix,
                files.len(),
                names.join(", "),
                total
            );
        }
    }
}

/// Spawn a file monitoring task that scans the directory and sends changed files
/// to the outbound channel, and receives inbound files from the network.
/// Spawn a file monitoring task.
/// - Scans `sync_dir` every second and sends changed files to `outbound_tx`.
/// - Receives inbound file messages from `inbound_rx` and writes them to disk.
fn spawn_file_sync_task(
    sync_dir: PathBuf,
    max_bytes: u64,
    outbound_tx: mpsc::UnboundedSender<ClipboardContent>,
    mut inbound_rx: mpsc::UnboundedReceiver<ClipboardMessage>,
    client_id: Option<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let file_monitor = match FileMonitor::new(sync_dir, max_bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to create file monitor: {}", e);
                return;
            }
        };

        println!("File sync active, scanning every 1s");

        // Use a dedicated thread for file scanning to avoid being starved
        // by blocking clipboard operations (wl-paste) on the tokio runtime.
        // Both scanning and writing share a single FileMonitor via channels
        // to keep hashes and suppression state consistent.
        let (scan_result_tx, mut scan_result_rx) =
            mpsc::unbounded_channel::<Vec<ClipboardContent>>();
        let (write_req_tx, write_req_rx) =
            std::sync::mpsc::channel::<(String, String, u64)>();

        let scan_dir = file_monitor.sync_dir().to_path_buf();
        let scan_max = file_monitor.max_file_size();
        std::thread::spawn(move || {
            let mut monitor = match FileMonitor::new(scan_dir, scan_max) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Failed to create file scanner: {}", e);
                    return;
                }
            };
            loop {
                // Process any pending writes first
                while let Ok((path, data, size)) = write_req_rx.try_recv() {
                    if let Err(e) = monitor.write_received_file(&path, &data, size) {
                        eprintln!("Failed to write received file: {}", e);
                    } else {
                        println!("Received file \"{}\" ({} bytes)", path, size);
                    }
                }

                std::thread::sleep(std::time::Duration::from_secs(1));
                let changes = monitor.scan_changes();
                if !changes.is_empty() {
                    if scan_result_tx.send(changes).is_err() {
                        break;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                Some(changes) = scan_result_rx.recv() => {
                    for content in changes {
                        log_content("File changed, syncing", &content);
                        if let Err(e) = outbound_tx.send(content) {
                            eprintln!("Failed to send file: {}", e);
                        }
                    }
                }
                Some(message) = inbound_rx.recv() => {
                    if let Some(ref our_id) = client_id {
                        if message.client_id.as_ref() == Some(our_id) {
                            continue;
                        }
                    }
                    if let ClipboardContent::File { ref path, ref data, size } = message.content {
                        let _ = write_req_tx.send((path.clone(), data.clone(), size));
                    }
                }
            }
        }
    })
}

async fn run_server(
    addr: SocketAddr,
    relay_only: bool,
    token: Option<String>,
    secret: Option<String>,
    cert: Option<String>,
    key: Option<String>,
    tls_auto_cert: bool,
    sync_dir: Option<PathBuf>,
    max_file_size: u64,
) -> Result<()> {
    println!("Starting clipboard sync server...");
    println!("Platform: {}", std::env::consts::OS);

    if relay_only {
        println!("Running in relay-only mode (no clipboard access)");
    }

    if let Some(ref dir) = sync_dir {
        println!(
            "File sync enabled: {} (max {} MB)",
            dir.display(),
            max_file_size
        );
    }

    // Build TLS acceptor if configured
    let tls_acceptor = if tls_auto_cert {
        println!("Generating self-signed TLS certificate...");
        let (cert_pem, key_pem) = tls::generate_self_signed_cert()?;
        println!("Self-signed certificate generated (for development use only)");
        Some(tls::build_server_tls_from_pem(&cert_pem, &key_pem)?)
    } else if let (Some(cert_path), Some(key_path)) = (&cert, &key) {
        Some(tls::build_server_tls(cert_path, key_path)?)
    } else if cert.is_some() || key.is_some() {
        anyhow::bail!("Both --cert and --key must be provided together");
    } else {
        None
    };

    let (tx, mut rx) = mpsc::unbounded_channel();
    let (broadcast_tx, _) = broadcast::channel::<ClipboardMessage>(100);

    let server = SyncServer::new(addr, tx.clone(), broadcast_tx.clone(), token, secret, tls_acceptor);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // File sync channels
    let (file_inbound_tx, file_inbound_rx) = mpsc::unbounded_channel::<ClipboardMessage>();
    let (file_outbound_tx, mut file_outbound_rx) = mpsc::unbounded_channel::<ClipboardContent>();
    let file_handle = sync_dir.map(|dir| {
        let max_bytes = max_file_size * 1024 * 1024;
        let sync_handle =
            spawn_file_sync_task(dir, max_bytes, file_outbound_tx, file_inbound_rx, None);
        // Bridge: forward outbound file content to the main broadcast as ClipboardMessages
        let file_broadcast_tx = broadcast_tx.clone();
        let bridge_handle = tokio::spawn(async move {
            while let Some(content) = file_outbound_rx.recv().await {
                let message = ClipboardMessage {
                    content,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    client_id: None,
                };
                if let Err(e) = file_broadcast_tx.send(message) {
                    eprintln!("Failed to broadcast file: {}", e);
                }
            }
        });
        (sync_handle, bridge_handle)
    });

    if relay_only {
        let receive_handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                log_content("Received from client, relaying", &message.content);
                // Forward File messages to file sync task
                if matches!(&message.content, ClipboardContent::File { .. }) {
                    let _ = file_inbound_tx.send(message.clone());
                }
                if let Err(e) = broadcast_tx.send(message) {
                    eprintln!("Failed to broadcast: {}", e);
                }
            }
        });

        if let Some((sh, bh)) = file_handle {
            tokio::try_join!(server_handle, receive_handle, sh, bh)?;
        } else {
            tokio::try_join!(server_handle, receive_handle)?;
        }
    } else {
        let clipboard_handle = tokio::spawn(async move {
            let mut clipboard = match ClipboardMonitor::new(Some(max_file_size * 1024 * 1024)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create clipboard monitor: {}", e);
                    return;
                }
            };

            let (local_tx, mut local_rx) = mpsc::unbounded_channel();

            let monitor_handle = {
                let local_tx = local_tx.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        if let Err(e) = local_tx.send(()) {
                            eprintln!("Monitor channel closed: {}", e);
                            break;
                        }
                    }
                })
            };

            loop {
                tokio::select! {
                    Some(_) = local_rx.recv() => {
                        if let Ok(Some(content)) = clipboard.get_clipboard_content() {
                            log_content("Server clipboard changed, broadcasting", &content);
                            let message = ClipboardMessage {
                                content,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                client_id: None,
                            };
                            if let Err(e) = broadcast_tx.send(message) {
                                eprintln!("Failed to broadcast: {}", e);
                            }
                        }
                    }
                    Some(message) = rx.recv() => {
                        log_content("Received from client", &message.content);
                        match &message.content {
                            ClipboardContent::File { .. } => {
                                let _ = file_inbound_tx.send(message.clone());
                            }
                            _ => {
                                if let Err(e) = clipboard.set_clipboard_content(&message.content) {
                                    eprintln!("Failed to set server clipboard: {}", e);
                                }
                            }
                        }
                    }
                    else => break,
                }
            }

            monitor_handle.abort();
        });

        if let Some((sh, bh)) = file_handle {
            tokio::try_join!(server_handle, clipboard_handle, sh, bh)?;
        } else {
            tokio::try_join!(server_handle, clipboard_handle)?;
        }
    }

    Ok(())
}

async fn resolve_server(server: &str) -> Result<SocketAddr> {
    use tokio::net::lookup_host;

    // If it already parses as SocketAddr, use it directly
    if let Ok(addr) = server.parse::<SocketAddr>() {
        return Ok(addr);
    }

    // If it has a colon, treat as host:port
    let host_port = if server.contains(':') {
        server.to_string()
    } else {
        // Default to port 9527
        format!("{}:9527", server)
    };

    let mut addrs = lookup_host(&host_port).await?;
    addrs
        .next()
        .ok_or_else(|| anyhow::anyhow!("Could not resolve server address: {}", server))
}

async fn run_client(
    server_str: String,
    _listen_addr: SocketAddr,
    token: Option<String>,
    secret: Option<String>,
    tls_enabled: bool,
    ca_cert: Option<String>,
    tls_skip_verify: bool,
    sync_dir: Option<PathBuf>,
    max_file_size: u64,
) -> Result<()> {
    println!("Starting clipboard sync client...");
    println!("Platform: {}", std::env::consts::OS);

    let server_addr = resolve_server(&server_str).await?;
    println!("Connecting to server: {} ({})", server_str, server_addr);

    if let Some(ref dir) = sync_dir {
        println!(
            "File sync enabled: {} (max {} MB)",
            dir.display(),
            max_file_size
        );
    }

    // Build TLS connector if configured
    let (tls_connector, tls_server_name) = if tls_enabled || ca_cert.is_some() || tls_skip_verify {
        let connector = tls::build_client_tls(ca_cert.as_deref(), tls_skip_verify)?;
        let server_name = tls::parse_server_name(&server_str)?;
        println!("TLS encryption enabled");
        (Some(connector), Some(server_name))
    } else {
        (None, None)
    };

    // Generate unique client ID
    let client_id = format!(
        "{}-{}",
        std::env::consts::OS,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
    );
    println!("Client ID: {}", client_id);

    // Channel for sending content to server (broadcast for reconnection support)
    let (to_server_tx, _) = broadcast::channel::<ClipboardContent>(100);
    // Channel for receiving content from server
    let (from_server_tx, from_server_rx) = mpsc::unbounded_channel();

    let client = SyncClient::new(
        server_addr,
        client_id.clone(),
        token,
        secret,
        tls_connector,
        tls_server_name,
    );

    // Task to maintain connection with server (bidirectional)
    let to_server_for_connection = to_server_tx.clone();
    let connection_handle = tokio::spawn(async move {
        loop {
            let to_server_rx = to_server_for_connection.subscribe();
            match client
                .connect_bidirectional(from_server_tx.clone(), to_server_rx)
                .await
            {
                Ok(_) => {
                    println!("Connection closed, reconnecting...");
                }
                Err(e) => {
                    eprintln!("Connection error: {}, retrying in 5s...", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    // File sync task (independent from clipboard, runs on its own interval)
    let (file_inbound_tx, file_inbound_rx) = mpsc::unbounded_channel::<ClipboardMessage>();
    let (file_outbound_tx, mut file_outbound_rx) = mpsc::unbounded_channel::<ClipboardContent>();
    let file_handle = sync_dir.map(|dir| {
        let max_bytes = max_file_size * 1024 * 1024;
        let sync_handle = spawn_file_sync_task(
            dir,
            max_bytes,
            file_outbound_tx,
            file_inbound_rx,
            Some(client_id.clone()),
        );
        // Bridge: forward outbound file content to the to_server broadcast channel
        let file_to_server_tx = to_server_tx.clone();
        let bridge_handle = tokio::spawn(async move {
            while let Some(content) = file_outbound_rx.recv().await {
                if let Err(e) = file_to_server_tx.send(content) {
                    eprintln!("Failed to send file to server: {}", e);
                }
            }
        });
        (sync_handle, bridge_handle)
    });

    // Router task: receives all messages from server and dispatches to
    // clipboard or file sync. Runs on its own lightweight task so it's never
    // blocked by wl-paste or file I/O.
    let (clipboard_rx_tx, clipboard_rx_rx) = mpsc::unbounded_channel::<ClipboardMessage>();
    let client_id_for_router = client_id.clone();
    let router_handle = tokio::spawn(async move {
        let mut from_server_rx = from_server_rx;
        while let Some(message) = from_server_rx.recv().await {
            // Skip our own messages
            if message.client_id.as_ref() == Some(&client_id_for_router) {
                continue;
            }
            match &message.content {
                ClipboardContent::File { .. } => {
                    let _ = file_inbound_tx.send(message);
                }
                _ => {
                    let _ = clipboard_rx_tx.send(message);
                }
            }
        }
    });

    // Clipboard management task (may block on wl-paste — isolated from routing)
    let clipboard_handle = tokio::spawn(async move {
        let mut clipboard = match ClipboardMonitor::new(Some(max_file_size * 1024 * 1024)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to create clipboard monitor: {}", e);
                return;
            }
        };

        let (local_tx, mut local_rx) = mpsc::unbounded_channel();
        let mut clipboard_rx_rx = clipboard_rx_rx;

        let monitor_handle = {
            let local_tx = local_tx.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    if let Err(e) = local_tx.send(()) {
                        eprintln!("Monitor channel closed: {}", e);
                        break;
                    }
                }
            })
        };

        loop {
            tokio::select! {
                Some(_) = local_rx.recv() => {
                    if let Ok(Some(content)) = clipboard.get_clipboard_content() {
                        log_content("Local clipboard changed, sending to server", &content);
                        if let Err(e) = to_server_tx.send(content) {
                            eprintln!("Failed to send to server: {}", e);
                        }
                    }
                }
                Some(message) = clipboard_rx_rx.recv() => {
                    log_content("Received from server", &message.content);
                    if let Err(e) = clipboard.set_clipboard_content(&message.content) {
                        eprintln!("Failed to set clipboard: {}", e);
                    }
                }
                else => break,
            }
        }

        monitor_handle.abort();
    });

    if let Some((sh, bh)) = file_handle {
        tokio::try_join!(connection_handle, router_handle, clipboard_handle, sh, bh)?;
    } else {
        tokio::try_join!(connection_handle, router_handle, clipboard_handle)?;
    }

    Ok(())
}
