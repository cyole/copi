mod modules;

use anyhow::Result;
use clap::{Parser, Subcommand};
use modules::clipboard::ClipboardMonitor;
use modules::sync::{ClipboardContent, ClipboardMessage, SyncClient, SyncServer};
use modules::tls;
use std::net::SocketAddr;
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

        /// Path to TLS certificate file (PEM). Enables TLS when provided with --key.
        #[arg(long, env = "COPI_TLS_CERT")]
        cert: Option<String>,

        /// Path to TLS private key file (PEM). Enables TLS when provided with --cert.
        #[arg(long, env = "COPI_TLS_KEY")]
        key: Option<String>,

        /// Auto-generate a self-signed TLS certificate (for development/testing)
        #[arg(long)]
        tls_auto_cert: bool,
    },
    Client {
        #[arg(short, long)]
        server: SocketAddr,

        #[arg(short, long, default_value = "0.0.0.0:9528")]
        listen: SocketAddr,

        /// Authentication token for connecting to a token-protected server.
        /// Can also be set via COPI_TOKEN environment variable.
        #[arg(short, long, env = "COPI_TOKEN")]
        token: Option<String>,

        /// Enable TLS connection to server
        #[arg(long)]
        tls: bool,

        /// Path to CA certificate file (PEM) for verifying server's TLS certificate
        #[arg(long, env = "COPI_TLS_CA_CERT")]
        ca_cert: Option<String>,

        /// Skip TLS certificate verification (for self-signed certs)
        #[arg(long)]
        tls_skip_verify: bool,
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
            cert,
            key,
            tls_auto_cert,
        } => {
            run_server(addr, relay_only, token, cert, key, tls_auto_cert).await?;
        }
        Commands::Client {
            server,
            listen,
            token,
            tls,
            ca_cert,
            tls_skip_verify,
        } => {
            run_client(server, listen, token, tls, ca_cert, tls_skip_verify).await?;
        }
    }

    Ok(())
}

async fn run_server(
    addr: SocketAddr,
    relay_only: bool,
    token: Option<String>,
    cert: Option<String>,
    key: Option<String>,
    tls_auto_cert: bool,
) -> Result<()> {
    println!("Starting clipboard sync server...");
    println!("Platform: {}", std::env::consts::OS);

    if relay_only {
        println!("Running in relay-only mode (no clipboard access)");
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

    let server = SyncServer::new(addr, tx.clone(), broadcast_tx.clone(), token, tls_acceptor);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    if relay_only {
        let receive_handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match &message.content {
                    ClipboardContent::Text(text) => {
                        println!("Received clipboard content from client: text ({} bytes), relaying to other clients...", text.len());
                    }
                    ClipboardContent::Image { width, height, .. } => {
                        println!("Received clipboard content from client: image ({}x{}), relaying to other clients...", width, height);
                    }
                    ClipboardContent::Html { html, .. } => {
                        println!("Received clipboard content from client: html ({} bytes), relaying to other clients...", html.len());
                    }
                }
                if let Err(e) = broadcast_tx.send(message) {
                    eprintln!("Failed to broadcast: {}", e);
                }
            }
        });

        tokio::try_join!(server_handle, receive_handle)?;
    } else {
        let clipboard_handle = tokio::spawn(async move {
            let mut clipboard = match ClipboardMonitor::new() {
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
                            match &content {
                                ClipboardContent::Text(text) => {
                                    println!("Server clipboard changed: text ({} bytes), broadcasting to clients...", text.len());
                                }
                                ClipboardContent::Image { width, height, .. } => {
                                    println!("Server clipboard changed: image ({}x{}), broadcasting to clients...", width, height);
                                }
                                ClipboardContent::Html { html, .. } => {
                                    println!("Server clipboard changed: html ({} bytes), broadcasting to clients...", html.len());
                                }
                            }
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
                        match &message.content {
                            ClipboardContent::Text(text) => {
                                println!(
                                    "Received clipboard content from client: text ({} bytes)",
                                    text.len()
                                );
                            }
                            ClipboardContent::Image { width, height, .. } => {
                                println!(
                                    "Received clipboard content from client: image ({}x{})",
                                    width, height
                                );
                            }
                            ClipboardContent::Html { html, .. } => {
                                println!(
                                    "Received clipboard content from client: html ({} bytes)",
                                    html.len()
                                );
                            }
                        }
                        if let Err(e) = clipboard.set_clipboard_content(&message.content) {
                            eprintln!("Failed to set server clipboard: {}", e);
                        }
                    }
                    else => break,
                }
            }

            monitor_handle.abort();
        });

        tokio::try_join!(server_handle, clipboard_handle)?;
    }

    Ok(())
}

async fn run_client(
    server_addr: SocketAddr,
    _listen_addr: SocketAddr,
    token: Option<String>,
    tls_enabled: bool,
    ca_cert: Option<String>,
    tls_skip_verify: bool,
) -> Result<()> {
    println!("Starting clipboard sync client...");
    println!("Platform: {}", std::env::consts::OS);
    println!("Connecting to server: {}", server_addr);

    // Build TLS connector if configured
    let (tls_connector, tls_server_name) = if tls_enabled || ca_cert.is_some() || tls_skip_verify {
        let connector = tls::build_client_tls(ca_cert.as_deref(), tls_skip_verify)?;
        let server_name = tls::parse_server_name(&server_addr.to_string())?;
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

    // Channel for sending clipboard content to server (broadcast for reconnection support)
    let (to_server_tx, _) = broadcast::channel::<ClipboardContent>(100);
    // Channel for receiving clipboard content from server
    let (from_server_tx, from_server_rx) = mpsc::unbounded_channel();

    let client = SyncClient::new(
        server_addr,
        client_id.clone(),
        token,
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

    // Unified clipboard management task
    let client_id_for_clipboard = client_id.clone();
    let clipboard_handle = tokio::spawn(async move {
        let mut clipboard = match ClipboardMonitor::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to create clipboard monitor: {}", e);
                return;
            }
        };

        let (local_tx, mut local_rx) = mpsc::unbounded_channel();
        let mut from_server_rx = from_server_rx;

        // Spawn clipboard monitoring task
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
                // Check local clipboard changes
                Some(_) = local_rx.recv() => {
                    if let Ok(Some(content)) = clipboard.get_clipboard_content() {
                        match &content {
                            ClipboardContent::Text(text) => {
                                println!(
                                    "Local clipboard changed, sending to server: text ({} bytes)",
                                    text.len()
                                );
                            }
                            ClipboardContent::Image { width, height, .. } => {
                                println!(
                                    "Local clipboard changed, sending to server: image ({}x{})",
                                    width, height
                                );
                            }
                            ClipboardContent::Html { html, .. } => {
                                println!(
                                    "Local clipboard changed, sending to server: html ({} bytes)",
                                    html.len()
                                );
                            }
                        }
                        if let Err(e) = to_server_tx.send(content) {
                            eprintln!("Failed to send to server: {}", e);
                        }
                    }
                }
                // Receive from server
                Some(message) = from_server_rx.recv() => {
                    // Skip messages from ourselves
                    if message.client_id.as_ref() == Some(&client_id_for_clipboard) {
                        continue;
                    }

                    match &message.content {
                        ClipboardContent::Text(text) => {
                            println!(
                                "Received clipboard from server: text ({} bytes)",
                                text.len()
                            );
                        }
                        ClipboardContent::Image { width, height, .. } => {
                            println!(
                                "Received clipboard from server: image ({}x{})",
                                width, height
                            );
                        }
                        ClipboardContent::Html { html, .. } => {
                            println!(
                                "Received clipboard from server: html ({} bytes)",
                                html.len()
                            );
                        }
                    }
                    // Update clipboard and hash together
                    if let Err(e) = clipboard.set_clipboard_content(&message.content) {
                        eprintln!("Failed to set clipboard: {}", e);
                    }
                }
                else => break,
            }
        }

        monitor_handle.abort();
    });

    tokio::try_join!(connection_handle, clipboard_handle)?;

    Ok(())
}
