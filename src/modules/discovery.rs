use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

const DISCOVERY_PORT: u16 = 9529;

#[derive(Serialize, Deserialize, Debug)]
struct DiscoveryPacket {
    #[serde(rename = "type")]
    msg_type: String, // "discover" or "announce"
    token_hash: String,
    client_id: String,
    listen_port: u16,
}

/// Discovered peer on the LAN.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub client_id: String,
    pub addr: SocketAddr,
}

/// Run LAN discovery: broadcast to find peers, listen for announcements.
/// Sends discovered peers to `peer_tx`. Runs until the task is cancelled.
pub async fn run_lan_discovery(
    token_hash: String,
    client_id: String,
    listen_port: u16,
    peer_tx: mpsc::UnboundedSender<DiscoveredPeer>,
) {
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)).await {
        Ok(s) => s,
        Err(_) => {
            // Port might be in use by another copi instance — try ephemeral
            match UdpSocket::bind("0.0.0.0:0").await {
                Ok(s) => {
                    eprintln!("LAN discovery: port {} in use, using ephemeral port", DISCOVERY_PORT);
                    s
                }
                Err(e) => {
                    eprintln!("LAN discovery: failed to bind UDP socket: {}", e);
                    return;
                }
            }
        }
    };

    if let Err(e) = socket.set_broadcast(true) {
        eprintln!("LAN discovery: failed to enable broadcast: {}", e);
        return;
    }

    let mut known_peers: HashSet<String> = HashSet::new();
    let mut buf = [0u8; 2048];

    println!("LAN discovery: broadcasting on port {}", DISCOVERY_PORT);

    loop {
        // Send discovery broadcast
        let discover = DiscoveryPacket {
            msg_type: "discover".to_string(),
            token_hash: token_hash.clone(),
            client_id: client_id.clone(),
            listen_port,
        };

        if let Ok(data) = serde_json::to_vec(&discover) {
            let broadcast_addr = format!("255.255.255.255:{}", DISCOVERY_PORT);
            let _ = socket.send_to(&data, &broadcast_addr).await;
        }

        // Listen for responses for 3 seconds
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(3);
        loop {
            let timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
            if timeout.is_zero() {
                break;
            }

            match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, src_addr))) => {
                    if let Ok(packet) = serde_json::from_slice::<DiscoveryPacket>(&buf[..len]) {
                        // Skip our own packets
                        if packet.client_id == client_id {
                            continue;
                        }
                        // Only respond to matching token group
                        if packet.token_hash != token_hash {
                            continue;
                        }

                        if packet.msg_type == "discover" {
                            // Respond with announce
                            let announce = DiscoveryPacket {
                                msg_type: "announce".to_string(),
                                token_hash: token_hash.clone(),
                                client_id: client_id.clone(),
                                listen_port,
                            };
                            if let Ok(data) = serde_json::to_vec(&announce) {
                                let _ = socket.send_to(&data, src_addr).await;
                            }
                        }

                        // For both discover and announce: track as peer
                        if !known_peers.contains(&packet.client_id) {
                            known_peers.insert(packet.client_id.clone());
                            let peer_addr = SocketAddr::new(src_addr.ip(), packet.listen_port);
                            println!(
                                "LAN discovery: found peer {} at {}",
                                &packet.client_id[..packet.client_id.len().min(16)],
                                peer_addr
                            );
                            let _ = peer_tx.send(DiscoveredPeer {
                                client_id: packet.client_id,
                                addr: peer_addr,
                            });
                        }
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => break, // timeout
            }
        }
    }
}
