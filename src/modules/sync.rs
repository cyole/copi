use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::TlsAcceptor;

use super::tls::{DynRead, DynWrite};

type HmacSha256 = Hmac<Sha256>;

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
    File {
        // Relative path within the sync directory
        path: String,
        // Base64-encoded file content
        data: String,
        // Original file size in bytes
        size: u64,
    },
    /// Files copied to clipboard via Ctrl+C in a file manager.
    /// Synced transparently so Ctrl+V works on the receiving machine.
    FileCopy {
        files: Vec<CopiedFile>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CopiedFile {
    pub name: String,
    pub data: String, // base64-encoded
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClipboardMessage {
    pub content: ClipboardContent,
    pub timestamp: u64,
    #[serde(default)]
    pub client_id: Option<String>,
}

// Auth protocol messages
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthChallenge {
    pub nonce: String, // hex-encoded 32-byte random nonce
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthRequest {
    pub hmac: String, // hex-encoded HMAC-SHA256(token, nonce)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
}

// Helper functions for length-prefixed message protocol (generic over any async reader/writer)
async fn read_message<T: for<'de> Deserialize<'de>, R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<T> {
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

async fn write_message<T: Serialize, W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &T,
) -> Result<()> {
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

/// Compute HMAC-SHA256(key=token, message=nonce_bytes) and return hex string.
fn compute_hmac(token: &str, nonce_bytes: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(token.as_bytes()).expect("HMAC can take key of any size");
    mac.update(nonce_bytes);
    hex::encode(&mac.finalize().into_bytes())
}

/// Hex encode/decode helpers (avoid adding a dependency just for this)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("Odd-length hex string".to_string());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
}

/// Run HMAC challenge-response authentication on the server side.
async fn server_auth_handshake(
    reader: &mut DynRead,
    writer: &mut DynWrite,
    expected_token: &str,
) -> Result<()> {
    // Generate random nonce
    let nonce_bytes: [u8; 32] = rand::thread_rng().gen();
    let nonce_hex = hex::encode(&nonce_bytes);

    // Send challenge
    let challenge = AuthChallenge { nonce: nonce_hex };
    write_message(writer, &challenge)
        .await
        .context("Failed to send auth challenge")?;

    // Read client's HMAC response
    let auth_req: AuthRequest = read_message(reader)
        .await
        .context("Failed to read auth request")?;

    // Compute expected HMAC and compare (constant-time via hmac crate)
    let mut mac = HmacSha256::new_from_slice(expected_token.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(&nonce_bytes);

    let client_hmac_bytes =
        hex::decode(&auth_req.hmac).map_err(|e| anyhow::anyhow!("Invalid HMAC hex: {}", e))?;

    if mac.verify_slice(&client_hmac_bytes).is_err() {
        let resp = AuthResponse {
            success: false,
            message: "Invalid token".to_string(),
        };
        let _ = write_message(writer, &resp).await;
        anyhow::bail!("Client provided invalid token");
    }

    let resp = AuthResponse {
        success: true,
        message: "Authenticated".to_string(),
    };
    write_message(writer, &resp)
        .await
        .context("Failed to send auth response")?;
    println!("Client authenticated successfully");

    Ok(())
}

/// Run HMAC challenge-response authentication on the client side.
async fn client_auth_handshake(
    reader: &mut DynRead,
    writer: &mut DynWrite,
    token: &str,
) -> Result<()> {
    // Read challenge from server
    let challenge: AuthChallenge = read_message(reader)
        .await
        .context("Failed to read auth challenge")?;

    let nonce_bytes =
        hex::decode(&challenge.nonce).map_err(|e| anyhow::anyhow!("Invalid nonce hex: {}", e))?;

    // Compute HMAC and send
    let hmac_hex = compute_hmac(token, &nonce_bytes);
    let auth_req = AuthRequest { hmac: hmac_hex };
    write_message(writer, &auth_req)
        .await
        .context("Failed to send auth request")?;

    // Read response
    let auth_resp: AuthResponse = read_message(reader)
        .await
        .context("Failed to read auth response")?;

    if !auth_resp.success {
        anyhow::bail!("Authentication failed: {}", auth_resp.message);
    }
    println!("Authenticated with server");

    Ok(())
}

pub struct SyncServer {
    addr: SocketAddr,
    tx: mpsc::UnboundedSender<ClipboardMessage>,
    broadcast_tx: broadcast::Sender<ClipboardMessage>,
    token: Option<String>,
    tls_acceptor: Option<TlsAcceptor>,
}

impl SyncServer {
    pub fn new(
        addr: SocketAddr,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        broadcast_tx: broadcast::Sender<ClipboardMessage>,
        token: Option<String>,
        tls_acceptor: Option<TlsAcceptor>,
    ) -> Self {
        Self {
            addr,
            tx,
            broadcast_tx,
            token,
            tls_acceptor,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("Server listening on {}", self.addr);
        if self.token.is_some() {
            println!("Token authentication enabled (HMAC challenge-response)");
        }
        if self.tls_acceptor.is_some() {
            println!("TLS encryption enabled");
        }

        loop {
            let (socket, addr) = listener.accept().await?;
            println!("New connection from {}", addr);

            let tx = self.tx.clone();
            let broadcast_rx = self.broadcast_tx.subscribe();
            let token = self.token.clone();
            let tls_acceptor = self.tls_acceptor.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_client(socket, tx, broadcast_rx, token, tls_acceptor).await
                {
                    eprintln!("Error handling client {}: {}", addr, e);
                }
            });
        }
    }

    async fn handle_client(
        socket: TcpStream,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        mut broadcast_rx: broadcast::Receiver<ClipboardMessage>,
        token: Option<String>,
        tls_acceptor: Option<TlsAcceptor>,
    ) -> Result<()> {
        // Optionally upgrade to TLS
        let (mut reader, mut writer): (DynRead, DynWrite) = if let Some(acceptor) = tls_acceptor {
            let tls_stream = acceptor
                .accept(socket)
                .await
                .context("TLS handshake failed")?;
            let (r, w) = tokio::io::split(tls_stream);
            (Box::new(r), Box::new(w))
        } else {
            let (r, w) = socket.into_split();
            (Box::new(r), Box::new(w))
        };

        // HMAC authentication handshake (runs inside TLS tunnel if enabled)
        if let Some(expected_token) = token {
            server_auth_handshake(&mut reader, &mut writer, &expected_token).await?;
        }

        // Task to receive messages from client
        let receive_handle = tokio::spawn(async move {
            loop {
                match read_message::<ClipboardMessage, _>(&mut reader).await {
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
                        if let Err(e) = write_message(&mut writer, &message).await {
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
    token: Option<String>,
    tls_connector: Option<tokio_rustls::TlsConnector>,
    tls_server_name: Option<rustls::pki_types::ServerName<'static>>,
}

impl SyncClient {
    pub fn new(
        addr: SocketAddr,
        client_id: String,
        token: Option<String>,
        tls_connector: Option<tokio_rustls::TlsConnector>,
        tls_server_name: Option<rustls::pki_types::ServerName<'static>>,
    ) -> Self {
        Self {
            addr,
            client_id,
            token,
            tls_connector,
            tls_server_name,
        }
    }

    pub async fn connect_bidirectional(
        &self,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        mut rx: broadcast::Receiver<ClipboardContent>,
    ) -> Result<()> {
        let stream = TcpStream::connect(self.addr).await?;
        println!("Connected to server at {}", self.addr);

        // Optionally upgrade to TLS
        let (mut reader, mut writer): (DynRead, DynWrite) =
            if let (Some(connector), Some(server_name)) =
                (&self.tls_connector, &self.tls_server_name)
            {
                let tls_stream = connector
                    .connect(server_name.clone(), stream)
                    .await
                    .context("TLS handshake failed")?;
                let (r, w) = tokio::io::split(tls_stream);
                (Box::new(r), Box::new(w))
            } else {
                let (r, w) = stream.into_split();
                (Box::new(r), Box::new(w))
            };

        // HMAC authentication handshake (runs inside TLS tunnel if enabled)
        if let Some(ref token) = self.token {
            client_auth_handshake(&mut reader, &mut writer, token).await?;
        }

        // Task to receive messages from server
        let receive_handle = tokio::spawn(async move {
            loop {
                match read_message::<ClipboardMessage, _>(&mut reader).await {
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

                        if let Err(e) = write_message(&mut writer, &message).await {
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
