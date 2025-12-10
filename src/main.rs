mod modules;

use anyhow::Result;
use clap::{Parser, Subcommand};
use modules::clipboard::ClipboardMonitor;
use modules::sync::{ClipboardContent, ClipboardMessage, SyncClient, SyncServer};
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

        /// 只转发模式：不访问剪贴板，仅在客户端之间转发数据（适用于无图形界面的服务器）
        #[arg(short, long)]
        relay_only: bool,
    },
    Client {
        #[arg(short, long)]
        server: SocketAddr,

        #[arg(short, long, default_value = "0.0.0.0:9528")]
        listen: SocketAddr,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { addr, relay_only } => {
            run_server(addr, relay_only).await?;
        }
        Commands::Client { server, listen } => {
            run_client(server, listen).await?;
        }
    }

    Ok(())
}

async fn run_server(addr: SocketAddr, relay_only: bool) -> Result<()> {
    println!("Starting clipboard sync server...");
    println!("Platform: {}", std::env::consts::OS);

    if relay_only {
        println!("Running in relay-only mode (no clipboard access)");
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    let (broadcast_tx, _) = broadcast::channel::<ClipboardMessage>(100);

    let server = SyncServer::new(addr, tx.clone(), broadcast_tx.clone());

    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    if relay_only {
        // 只转发模式：只接收来自客户端的消息并转发，不访问剪贴板
        let receive_handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match &message.content {
                    ClipboardContent::Text(text) => {
                        println!("Received clipboard content from client: text ({} bytes), relaying to other clients...", text.len());
                    }
                    ClipboardContent::Image { width, height, .. } => {
                        println!("Received clipboard content from client: image ({}x{}), relaying to other clients...", width, height);
                    }
                }
                // 在只转发模式下，通过 broadcast 发送给其他客户端（保留 client_id）
                if let Err(e) = broadcast_tx.send(message) {
                    eprintln!("Failed to broadcast: {}", e);
                }
            }
        });

        tokio::try_join!(server_handle, receive_handle)?;
    } else {
        // 正常模式：访问剪贴板
        let broadcast_for_clipboard = broadcast_tx.clone();
        let clipboard_handle = tokio::spawn(async move {
            let mut clipboard = match ClipboardMonitor::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create clipboard monitor: {}", e);
                    return;
                }
            };

            if let Err(e) = clipboard.monitor(move |content| {
                match &content {
                    ClipboardContent::Text(text) => {
                        println!("Server clipboard changed: text ({} bytes), broadcasting to clients...", text.len());
                    }
                    ClipboardContent::Image { width, height, .. } => {
                        println!("Server clipboard changed: image ({}x{}), broadcasting to clients...", width, height);
                    }
                }
                let message = ClipboardMessage {
                    content: content.clone(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    client_id: None, // 服务器本地的剪贴板变化没有 client_id
                };
                if let Err(e) = broadcast_for_clipboard.send(message) {
                    eprintln!("Failed to broadcast: {}", e);
                }
                Ok(())
            }).await {
                eprintln!("Clipboard monitor error: {}", e);
            }
        });

        let receive_handle = tokio::spawn(async move {
            let mut clipboard = match ClipboardMonitor::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create clipboard monitor for receiving: {}", e);
                    return;
                }
            };

            while let Some(message) = rx.recv().await {
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
                }
                // Update server's clipboard when receiving from client
                if let Err(e) = clipboard.set_clipboard_content(&message.content) {
                    eprintln!("Failed to set server clipboard: {}", e);
                }
            }
        });

        tokio::try_join!(server_handle, clipboard_handle, receive_handle)?;
    }

    Ok(())
}

async fn run_client(server_addr: SocketAddr, _listen_addr: SocketAddr) -> Result<()> {
    println!("Starting clipboard sync client...");
    println!("Platform: {}", std::env::consts::OS);
    println!("Connecting to server: {}", server_addr);

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

    let client = SyncClient::new(server_addr, client_id.clone());

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
    // This task handles both monitoring local changes and receiving from server
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
