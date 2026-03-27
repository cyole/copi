use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl ScreenEdge {
    /// Returns the opposite (mirror) edge.
    /// When a cursor exits one edge, it enters the peer from the opposite edge.
    pub fn mirror(&self) -> ScreenEdge {
        match self {
            ScreenEdge::Left => ScreenEdge::Right,
            ScreenEdge::Right => ScreenEdge::Left,
            ScreenEdge::Top => ScreenEdge::Bottom,
            ScreenEdge::Bottom => ScreenEdge::Top,
        }
    }

    pub fn parse(s: &str) -> Result<ScreenEdge, String> {
        match s.to_lowercase().as_str() {
            "left" => Ok(ScreenEdge::Left),
            "right" => Ok(ScreenEdge::Right),
            "top" => Ok(ScreenEdge::Top),
            "bottom" => Ok(ScreenEdge::Bottom),
            _ => Err(format!("Invalid screen edge: '{}'. Use left, right, top, or bottom", s)),
        }
    }
}

impl std::fmt::Display for ScreenEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScreenEdge::Left => write!(f, "left"),
            ScreenEdge::Right => write!(f, "right"),
            ScreenEdge::Top => write!(f, "top"),
            ScreenEdge::Bottom => write!(f, "bottom"),
        }
    }
}

/// Screen geometry and edge-to-peer mapping.
pub struct ScreenConfig {
    pub width: u32,
    pub height: u32,
    /// Maps a local screen edge to the peer_client_id that should receive the cursor.
    pub edge_peers: HashMap<ScreenEdge, String>,
}

/// Threshold in pixels for edge detection.
const EDGE_THRESHOLD: f64 = 1.0;

impl ScreenConfig {
    /// Detect local screen dimensions using rdev.
    pub fn detect() -> Self {
        let (width, height) = rdev::display_size().unwrap_or((1920, 1080));
        Self {
            width: width as u32,
            height: height as u32,
            edge_peers: HashMap::new(),
        }
    }

    /// Create with known dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            edge_peers: HashMap::new(),
        }
    }

    /// Assign a peer to a screen edge.
    pub fn set_peer_edge(&mut self, edge: ScreenEdge, peer_id: String) {
        self.edge_peers.insert(edge, peer_id);
    }

    /// Check if position (x, y) in absolute pixels hits a configured edge.
    /// Returns the edge and the peer_id associated with it.
    pub fn edge_hit(&self, x: f64, y: f64) -> Option<(ScreenEdge, &str)> {
        let w = self.width as f64;
        let h = self.height as f64;

        // Check each configured edge
        if x <= EDGE_THRESHOLD {
            if let Some(peer) = self.edge_peers.get(&ScreenEdge::Left) {
                return Some((ScreenEdge::Left, peer));
            }
        }
        if x >= w - EDGE_THRESHOLD {
            if let Some(peer) = self.edge_peers.get(&ScreenEdge::Right) {
                return Some((ScreenEdge::Right, peer));
            }
        }
        if y <= EDGE_THRESHOLD {
            if let Some(peer) = self.edge_peers.get(&ScreenEdge::Top) {
                return Some((ScreenEdge::Top, peer));
            }
        }
        if y >= h - EDGE_THRESHOLD {
            if let Some(peer) = self.edge_peers.get(&ScreenEdge::Bottom) {
                return Some((ScreenEdge::Bottom, peer));
            }
        }

        None
    }

    /// Convert absolute pixel position to normalized [0.0, 1.0] coordinates.
    pub fn normalize(&self, x: f64, y: f64) -> (f64, f64) {
        let w = self.width as f64;
        let h = self.height as f64;
        (
            (x / w).clamp(0.0, 1.0),
            (y / h).clamp(0.0, 1.0),
        )
    }

    /// Convert normalized [0.0, 1.0] coordinates to absolute pixel position.
    pub fn denormalize(&self, nx: f64, ny: f64) -> (f64, f64) {
        (nx * self.width as f64, ny * self.height as f64)
    }

    /// Compute the entry position on the receiving screen given the exit edge
    /// and the normalized position along that edge.
    ///
    /// Mirror rule: exit left -> enter right, exit right -> enter left, etc.
    /// The position along the edge is preserved (e.g., 30% down left -> 30% down right).
    pub fn entry_position(exit_edge: ScreenEdge, x_norm: f64, y_norm: f64) -> (f64, f64) {
        match exit_edge {
            // Exiting left edge -> entering from right side of peer screen
            ScreenEdge::Left => (1.0, y_norm),
            // Exiting right edge -> entering from left side of peer screen
            ScreenEdge::Right => (0.0, y_norm),
            // Exiting top edge -> entering from bottom of peer screen
            ScreenEdge::Top => (x_norm, 1.0),
            // Exiting bottom edge -> entering from top of peer screen
            ScreenEdge::Bottom => (x_norm, 0.0),
        }
    }

    /// Check if a normalized position is at the entry edge (cursor returning).
    /// The entry edge is the mirror of the exit edge.
    pub fn is_at_entry_edge(entry_edge: ScreenEdge, nx: f64, ny: f64) -> bool {
        match entry_edge {
            ScreenEdge::Left => nx <= 0.01,
            ScreenEdge::Right => nx >= 0.99,
            ScreenEdge::Top => ny <= 0.01,
            ScreenEdge::Bottom => ny >= 0.99,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mirror_edges() {
        assert_eq!(ScreenEdge::Left.mirror(), ScreenEdge::Right);
        assert_eq!(ScreenEdge::Right.mirror(), ScreenEdge::Left);
        assert_eq!(ScreenEdge::Top.mirror(), ScreenEdge::Bottom);
        assert_eq!(ScreenEdge::Bottom.mirror(), ScreenEdge::Top);
    }

    #[test]
    fn test_normalize_denormalize_roundtrip() {
        let config = ScreenConfig::new(1920, 1080);
        let (nx, ny) = config.normalize(960.0, 540.0);
        assert!((nx - 0.5).abs() < 0.001);
        assert!((ny - 0.5).abs() < 0.001);
        let (x, y) = config.denormalize(nx, ny);
        assert!((x - 960.0).abs() < 1.0);
        assert!((y - 540.0).abs() < 1.0);
    }

    #[test]
    fn test_entry_position_mirror() {
        // Exit right at 40% height -> enter left at 40% height
        let (ex, ey) = ScreenConfig::entry_position(ScreenEdge::Right, 1.0, 0.4);
        assert!((ex - 0.0).abs() < 0.001);
        assert!((ey - 0.4).abs() < 0.001);

        // Exit left at 30% height -> enter right at 30% height
        let (ex, ey) = ScreenConfig::entry_position(ScreenEdge::Left, 0.0, 0.3);
        assert!((ex - 1.0).abs() < 0.001);
        assert!((ey - 0.3).abs() < 0.001);

        // Exit top at 50% width -> enter bottom at 50% width
        let (ex, ey) = ScreenConfig::entry_position(ScreenEdge::Top, 0.5, 0.0);
        assert!((ex - 0.5).abs() < 0.001);
        assert!((ey - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_edge_hit() {
        let mut config = ScreenConfig::new(1920, 1080);
        config.set_peer_edge(ScreenEdge::Right, "peer-1".to_string());

        // At right edge
        assert!(config.edge_hit(1919.5, 500.0).is_some());
        // Not at right edge
        assert!(config.edge_hit(1000.0, 500.0).is_none());
        // At left edge but no peer configured
        assert!(config.edge_hit(0.0, 500.0).is_none());
    }

    #[test]
    fn test_parse_edge() {
        assert_eq!(ScreenEdge::parse("left").unwrap(), ScreenEdge::Left);
        assert_eq!(ScreenEdge::parse("RIGHT").unwrap(), ScreenEdge::Right);
        assert_eq!(ScreenEdge::parse("Top").unwrap(), ScreenEdge::Top);
        assert!(ScreenEdge::parse("invalid").is_err());
    }
}
