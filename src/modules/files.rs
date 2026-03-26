use anyhow::Result;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::modules::sync::ClipboardContent;

pub struct FileMonitor {
    sync_dir: PathBuf,
    max_file_size: u64,
    /// SHA-256 hashes of tracked files: relative_path -> hash
    file_hashes: HashMap<String, String>,
    /// Files recently written by the network receiver, suppressed from re-sending.
    /// Maps relative path -> time written
    suppressed: HashMap<String, Instant>,
}

const SUPPRESS_DURATION: Duration = Duration::from_secs(2);

impl FileMonitor {
    pub fn new(sync_dir: PathBuf, max_file_size: u64) -> Result<Self> {
        std::fs::create_dir_all(&sync_dir)?;
        let sync_dir = sync_dir
            .canonicalize()
            .unwrap_or(sync_dir);
        Ok(Self {
            sync_dir,
            max_file_size,
            file_hashes: HashMap::new(),
            suppressed: HashMap::new(),
        })
    }

    pub fn sync_dir(&self) -> &Path {
        &self.sync_dir
    }

    pub fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    /// Scan the sync directory and return any new or modified files.
    pub fn scan_changes(&mut self) -> Vec<ClipboardContent> {
        // Clean up expired suppressions
        self.suppressed
            .retain(|_, t| t.elapsed() < SUPPRESS_DURATION);

        let mut changed = Vec::new();
        let mut current_files: HashMap<String, String> = HashMap::new();

        let entries = match walk_dir(&self.sync_dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Failed to walk sync dir {}: {}", self.sync_dir.display(), e);
                return changed;
            }
        };

        for entry in entries {
            let rel_path = match entry.strip_prefix(&self.sync_dir) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };

            // Skip suppressed files (recently written from network)
            if self.suppressed.contains_key(&rel_path) {
                if let Ok(hash) = hash_file(&entry) {
                    current_files.insert(rel_path, hash);
                }
                continue;
            }

            // Check file size
            let metadata = match std::fs::metadata(&entry) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !metadata.is_file() {
                continue;
            }
            let size = metadata.len();
            if size > self.max_file_size {
                continue;
            }

            // Hash the file
            let hash = match hash_file(&entry) {
                Ok(h) => h,
                Err(_) => continue,
            };

            let is_new_or_changed = self
                .file_hashes
                .get(&rel_path)
                .map_or(true, |old_hash| old_hash != &hash);

            current_files.insert(rel_path.clone(), hash);

            if is_new_or_changed {
                if let Ok(data) = std::fs::read(&entry) {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                    changed.push(ClipboardContent::File {
                        path: rel_path,
                        data: encoded,
                        size,
                    });
                }
            }
        }

        self.file_hashes = current_files;
        changed
    }

    /// Write a received file to the sync directory.
    /// Suppresses the file from being re-scanned as a change.
    pub fn write_received_file(&mut self, path: &str, data: &str, _size: u64) -> Result<()> {
        // Sanitize path: prevent directory traversal
        let rel = Path::new(path);
        if rel.is_absolute() || rel.components().any(|c| c == std::path::Component::ParentDir) {
            anyhow::bail!("Invalid file path: {}", path);
        }

        let full_path = self.sync_dir.join(rel);

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| anyhow::anyhow!("Failed to decode file data: {}", e))?;

        std::fs::write(&full_path, &bytes)?;

        // Suppress this file from re-scanning and update hash
        let rel_str = path.replace('\\', "/");
        self.suppressed.insert(rel_str.clone(), Instant::now());

        let hash = hash_bytes(&bytes);
        self.file_hashes.insert(rel_str, hash);

        Ok(())
    }
}

fn hash_file(path: &Path) -> Result<String> {
    let data = std::fs::read(path)?;
    Ok(hash_bytes(&data))
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Recursively walk a directory and collect file paths.
fn walk_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk_dir(&path)?);
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}
