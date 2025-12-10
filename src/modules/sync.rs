use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClipboardContent {
    Text(String),
    Image {
        // PNG format, base64 encoded
        data: String,
        width: u32,
        height: u32,
    },
    Html {
        // HTML content
        html: String,
        // Plain text fallback
        #[serde(default)]
        text: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClipboardMessage {
    pub content: ClipboardContent,
    pub timestamp: u64,
    #[serde(default)]
    pub client_id: Option<String>,
}

// Helper functions for length-prefixed message protocol
async fn read_message<T: for<'de> Deserialize<'de>>(reader: &mut OwnedReadHalf) -> Result<T> {
    // Read 4-byte length prefix (big-endian)
    let mut len_bytes = [0u8; 4];
    reader
        .read_exact(&mut len_bytes)
        .await
        .context("Failed to read message length")?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    // Validate message length
    if len == 0 || len > 10_000_000 {
        // Max 10MB
        anyhow::bail!("Invalid message length: {}", len);
    }

    // Read message data
    let mut buffer = vec![0u8; len];
    reader
        .read_exact(&mut buffer)
        .await
        .context("Failed to read message data")?;

    // Deserialize JSON
    serde_json::from_slice(&buffer).context("Failed to deserialize message")
}

async fn write_message<T: Serialize>(writer: &mut OwnedWriteHalf, message: &T) -> Result<()> {
    // Serialize to JSON
    let data = serde_json::to_vec(message).context("Failed to serialize message")?;

    // Write length prefix (4 bytes, big-endian)
    let len = data.len() as u32;
    writer
        .write_all(&len.to_be_bytes())
        .await
        .context("Failed to write length prefix")?;

    // Write message data
    writer
        .write_all(&data)
        .await
        .context("Failed to write message data")?;

    writer.flush().await.context("Failed to flush")?;

    Ok(())
}

pub struct SyncServer {
    addr: SocketAddr,
    tx: mpsc::UnboundedSender<ClipboardMessage>,
    broadcast_tx: broadcast::Sender<ClipboardMessage>,
}

impl SyncServer {
    pub fn new(
        addr: SocketAddr,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        broadcast_tx: broadcast::Sender<ClipboardMessage>,
    ) -> Self {
        Self {
            addr,
            tx,
            broadcast_tx,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("Server listening on {}", self.addr);

        loop {
            let (socket, addr) = listener.accept().await?;
            println!("New connection from {}", addr);

            let tx = self.tx.clone();
            let broadcast_rx = self.broadcast_tx.subscribe();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(socket, tx, broadcast_rx).await {
                    eprintln!("Error handling client {}: {}", addr, e);
                }
            });
        }
    }

    async fn handle_client(
        socket: TcpStream,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        mut broadcast_rx: broadcast::Receiver<ClipboardMessage>,
    ) -> Result<()> {
        let (mut read_half, mut write_half) = socket.into_split();

        // Task to receive messages from client
        let receive_handle = tokio::spawn(async move {
            loop {
                match read_message::<ClipboardMessage>(&mut read_half).await {
                    Ok(message) => {
                        if let Err(e) = tx.send(message) {
                            eprintln!("Failed to send to channel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        if e.to_string().contains("Failed to read message length") {
                            // Connection closed
                            break;
                        }
                        eprintln!("Error reading from client: {}", e);
                        break;
                    }
                }
            }
        });

        // Task to broadcast messages to client
        let broadcast_handle = tokio::spawn(async move {
            loop {
                match broadcast_rx.recv().await {
                    Ok(message) => {
                        if let Err(e) = write_message(&mut write_half, &message).await {
                            eprintln!("Failed to write to client: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Broadcast receive error: {}", e);
                        break;
                    }
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = receive_handle => {},
            _ = broadcast_handle => {},
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct SyncClient {
    addr: SocketAddr,
    client_id: String,
}

impl SyncClient {
    pub fn new(addr: SocketAddr, client_id: String) -> Self {
        Self { addr, client_id }
    }

    pub async fn connect_bidirectional(
        &self,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        mut rx: broadcast::Receiver<ClipboardContent>,
    ) -> Result<()> {
        let stream = TcpStream::connect(self.addr).await?;
        println!("Connected to server at {}", self.addr);

        let (mut read_half, mut write_half) = stream.into_split();

        // Task to receive messages from server
        let receive_handle = tokio::spawn(async move {
            loop {
                match read_message::<ClipboardMessage>(&mut read_half).await {
                    Ok(message) => {
                        if let Err(e) = tx.send(message) {
                            eprintln!("Failed to send received message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        if e.to_string().contains("Failed to read message length") {
                            println!("Server closed connection");
                            break;
                        }
                        eprintln!("Error reading from server: {}", e);
                        break;
                    }
                }
            }
        });

        // Task to send messages to server
        let client_id = self.client_id.clone();
        let send_handle = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(content) => {
                        let message = ClipboardMessage {
                            content,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            client_id: Some(client_id.clone()),
                        };

                        if let Err(e) = write_message(&mut write_half, &message).await {
                            eprintln!("Failed to send to server: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Skip lagged messages
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = receive_handle => {},
            _ = send_handle => {},
        }

        Ok(())
    }
}
