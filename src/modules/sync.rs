use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio_rustls::TlsAcceptor;

use super::tls::{DynRead, DynWrite};

type HmacSha256 = Hmac<Sha256>;

/// Enable TCP keepalive on a socket to detect dead connections.
/// Sends a probe after 30s idle, then every 10s, gives up after 3 failed probes.
fn set_tcp_keepalive(stream: &TcpStream) {
    let sock_ref = socket2::SockRef::from(stream);
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(Duration::from_secs(30))
        .with_interval(Duration::from_secs(10));
    let _ = sock_ref.set_tcp_keepalive(&keepalive);
}

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
    pub hmac: String,       // hex-encoded HMAC-SHA256(token, nonce)
    #[serde(default)]
    pub token_hash: String, // hex-encoded SHA256(token) — used as group ID
    #[serde(default)]
    pub secret_hmac: String, // hex-encoded HMAC-SHA256(secret, nonce) — server gate auth
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

/// Compute SHA256(token) as hex — used as group identifier.
fn token_to_group_id(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(&hasher.finalize())
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

/// Run authentication on the server side. Returns the group ID.
/// Validates server secret first (if set), then token/group assignment.
async fn server_auth_handshake(
    reader: &mut DynRead,
    writer: &mut DynWrite,
    expected_token: Option<&str>,
    expected_secret: Option<&str>,
    multi_group: bool,
) -> Result<String> {
    // Generate random nonce
    let nonce_bytes: [u8; 32] = rand::thread_rng().gen();
    let nonce_hex = hex::encode(&nonce_bytes);

    // Send challenge
    let challenge = AuthChallenge { nonce: nonce_hex };
    write_message(writer, &challenge)
        .await
        .context("Failed to send auth challenge")?;

    // Read client's auth request
    let auth_req: AuthRequest = read_message(reader)
        .await
        .context("Failed to read auth request")?;

    // Step 1: Validate server secret (gate authentication)
    if let Some(secret) = expected_secret {
        if auth_req.secret_hmac.is_empty() {
            let resp = AuthResponse {
                success: false,
                message: "Server secret required".to_string(),
            };
            let _ = write_message(writer, &resp).await;
            anyhow::bail!("Client did not provide server secret");
        }

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(&nonce_bytes);

        let client_secret_bytes = hex::decode(&auth_req.secret_hmac)
            .map_err(|e| anyhow::anyhow!("Invalid secret HMAC hex: {}", e))?;

        if mac.verify_slice(&client_secret_bytes).is_err() {
            let resp = AuthResponse {
                success: false,
                message: "Invalid server secret".to_string(),
            };
            let _ = write_message(writer, &resp).await;
            anyhow::bail!("Client provided invalid server secret");
        }
    }

    // Step 2: Token / group assignment
    let group_id = if let Some(expected) = expected_token {
        // Single-token mode: verify HMAC
        let mut mac = HmacSha256::new_from_slice(expected.as_bytes())
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
        token_to_group_id(expected)
    } else if multi_group {
        // Multi-group mode: group by token_hash
        if auth_req.token_hash.is_empty() {
            let resp = AuthResponse {
                success: false,
                message: "Token required".to_string(),
            };
            let _ = write_message(writer, &resp).await;
            anyhow::bail!("Client did not provide token_hash");
        }
        auth_req.token_hash.clone()
    } else {
        "default".to_string()
    };

    let resp = AuthResponse {
        success: true,
        message: "Authenticated".to_string(),
    };
    write_message(writer, &resp)
        .await
        .context("Failed to send auth response")?;

    let short_id = if group_id.len() >= 8 { &group_id[..8] } else { &group_id };
    println!("Client authenticated (group {}...)", short_id);

    Ok(group_id)
}

/// Run authentication on the client side.
async fn client_auth_handshake(
    reader: &mut DynRead,
    writer: &mut DynWrite,
    token: &str,
    secret: Option<&str>,
) -> Result<()> {
    // Read challenge from server
    let challenge: AuthChallenge = read_message(reader)
        .await
        .context("Failed to read auth challenge")?;

    let nonce_bytes =
        hex::decode(&challenge.nonce).map_err(|e| anyhow::anyhow!("Invalid nonce hex: {}", e))?;

    // Compute HMACs and send
    let auth_req = AuthRequest {
        hmac: compute_hmac(token, &nonce_bytes),
        token_hash: token_to_group_id(token),
        secret_hmac: secret
            .map(|s| compute_hmac(s, &nonce_bytes))
            .unwrap_or_default(),
    };
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

/// Thread-safe map of group_id -> broadcast channel.
type GroupMap = Arc<RwLock<HashMap<String, broadcast::Sender<ClipboardMessage>>>>;

/// Rate limiter: tracks failed auth attempts per IP.
const MAX_FAILURES: u32 = 10;
const RATE_LIMIT_WINDOW_SECS: u64 = 600; // 10 minutes

#[derive(Clone)]
struct RateLimiter {
    failures: Arc<Mutex<HashMap<IpAddr, (u32, Instant)>>>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            failures: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn is_blocked(&self, ip: IpAddr) -> bool {
        let map = self.failures.lock().await;
        if let Some((count, first_failure)) = map.get(&ip) {
            if first_failure.elapsed().as_secs() < RATE_LIMIT_WINDOW_SECS {
                return *count >= MAX_FAILURES;
            }
        }
        false
    }

    async fn record_failure(&self, ip: IpAddr) {
        let mut map = self.failures.lock().await;
        let entry = map.entry(ip).or_insert((0, Instant::now()));
        if entry.1.elapsed().as_secs() >= RATE_LIMIT_WINDOW_SECS {
            // Reset window
            *entry = (1, Instant::now());
        } else {
            entry.0 += 1;
        }
    }

    async fn clear(&self, ip: IpAddr) {
        let mut map = self.failures.lock().await;
        map.remove(&ip);
    }
}

pub struct SyncServer {
    addr: SocketAddr,
    tx: mpsc::UnboundedSender<ClipboardMessage>,
    broadcast_tx: broadcast::Sender<ClipboardMessage>,
    token: Option<String>,
    /// Server secret — required to connect at all (gate authentication).
    secret: Option<String>,
    tls_acceptor: Option<TlsAcceptor>,
    groups: GroupMap,
    rate_limiter: RateLimiter,
}

impl SyncServer {
    pub fn new(
        addr: SocketAddr,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        broadcast_tx: broadcast::Sender<ClipboardMessage>,
        token: Option<String>,
        secret: Option<String>,
        tls_acceptor: Option<TlsAcceptor>,
    ) -> Self {
        Self {
            addr,
            tx,
            broadcast_tx,
            token,
            secret,
            tls_acceptor,
            groups: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter: RateLimiter::new(),
        }
    }

    /// Get or create a broadcast channel for a group.
    async fn get_group_channel(
        groups: &GroupMap,
        group_id: &str,
    ) -> broadcast::Sender<ClipboardMessage> {
        // Fast path: read lock
        {
            let map = groups.read().await;
            if let Some(tx) = map.get(group_id) {
                return tx.clone();
            }
        }
        // Slow path: write lock, create new channel
        let mut map = groups.write().await;
        map.entry(group_id.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                println!("Created new clipboard group {}...", &group_id[..8]);
                tx
            })
            .clone()
    }

    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("Server listening on {}", self.addr);
        let multi_group = self.token.is_none();
        if multi_group {
            println!("Multi-group mode: each token = separate clipboard group");
        } else {
            println!("Single-group mode (HMAC challenge-response)");
        }
        if self.secret.is_some() {
            println!("Server secret required (rate limit: {} attempts per {} min)",
                MAX_FAILURES, RATE_LIMIT_WINDOW_SECS / 60);
        }
        if self.tls_acceptor.is_some() {
            println!("TLS encryption enabled");
        }

        loop {
            let (socket, addr) = listener.accept().await?;
            let client_ip = addr.ip();

            // Check rate limit before processing
            if self.rate_limiter.is_blocked(client_ip).await {
                eprintln!("Rate limited: {} (too many failed attempts)", addr);
                drop(socket);
                continue;
            }

            println!("New connection from {}", addr);

            let tx = self.tx.clone();
            let broadcast_tx = self.broadcast_tx.clone();
            let token = self.token.clone();
            let secret = self.secret.clone();
            let tls_acceptor = self.tls_acceptor.clone();
            let groups = self.groups.clone();
            let rate_limiter = self.rate_limiter.clone();
            let multi_group = multi_group;
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(
                    socket,
                    tx,
                    broadcast_tx,
                    token,
                    secret,
                    tls_acceptor,
                    groups,
                    multi_group,
                    client_ip,
                    rate_limiter.clone(),
                )
                .await
                {
                    eprintln!("Error handling client {}: {}", addr, e);
                }
            });
        }
    }

    async fn handle_client(
        socket: TcpStream,
        tx: mpsc::UnboundedSender<ClipboardMessage>,
        broadcast_tx: broadcast::Sender<ClipboardMessage>,
        token: Option<String>,
        secret: Option<String>,
        tls_acceptor: Option<TlsAcceptor>,
        groups: GroupMap,
        multi_group: bool,
        client_ip: IpAddr,
        rate_limiter: RateLimiter,
    ) -> Result<()> {
        set_tcp_keepalive(&socket);

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

        // Authenticate (with secret validation and rate limiting)
        let auth_result = server_auth_handshake(
            &mut reader,
            &mut writer,
            token.as_deref(),
            secret.as_deref(),
            multi_group,
        )
        .await;

        let group_id = match auth_result {
            Ok(gid) => {
                rate_limiter.clear(client_ip).await;
                gid
            }
            Err(e) => {
                rate_limiter.record_failure(client_ip).await;
                return Err(e);
            }
        };

        // Get the group's broadcast channel
        let group_broadcast_tx = if multi_group {
            Self::get_group_channel(&groups, &group_id).await
        } else {
            broadcast_tx
        };

        let mut group_broadcast_rx = group_broadcast_tx.subscribe();

        // In multi-group mode, relay directly within the group (no central relay).
        // In single-token mode, send to central mpsc for backward-compat relay.
        let use_central_relay = !multi_group;

        // Task to receive messages from client
        let group_tx_for_recv = group_broadcast_tx.clone();
        let receive_handle = tokio::spawn(async move {
            loop {
                match read_message::<ClipboardMessage, _>(&mut reader).await {
                    Ok(message) => {
                        if use_central_relay {
                            if let Err(e) = tx.send(message) {
                                eprintln!("Failed to send to channel: {}", e);
                                break;
                            }
                        } else {
                            // Multi-group: broadcast directly within the group
                            if let Err(e) = group_tx_for_recv.send(message) {
                                eprintln!("Failed to broadcast to group: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        if e.to_string().contains("Failed to read message length") {
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
                match group_broadcast_rx.recv().await {
                    Ok(message) => {
                        if let Err(e) = write_message(&mut writer, &message).await {
                            eprintln!("Failed to write to client: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(e) => {
                        eprintln!("Broadcast receive error: {}", e);
                        break;
                    }
                }
            }
        });

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
    secret: Option<String>,
    tls_connector: Option<tokio_rustls::TlsConnector>,
    tls_server_name: Option<rustls::pki_types::ServerName<'static>>,
}

impl SyncClient {
    pub fn new(
        addr: SocketAddr,
        client_id: String,
        token: Option<String>,
        secret: Option<String>,
        tls_connector: Option<tokio_rustls::TlsConnector>,
        tls_server_name: Option<rustls::pki_types::ServerName<'static>>,
    ) -> Self {
        Self {
            addr,
            client_id,
            token,
            secret,
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
        set_tcp_keepalive(&stream);
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

        // Authentication handshake (runs inside TLS tunnel if enabled)
        if let Some(ref token) = self.token {
            client_auth_handshake(&mut reader, &mut writer, token, self.secret.as_deref()).await?;
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
