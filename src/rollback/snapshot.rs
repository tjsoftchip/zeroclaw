//! Configuration snapshot data structures and manager.
//!
//! Provides the core snapshot functionality for OpenWrt configuration
//! backup and rollback operations.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::store::SnapshotStore;

pub const DEFAULT_CONFIG_DIR: &str = "/etc/config";
pub const SNAPSHOT_DIR_NAME: &str = "snapshots";
pub const METADATA_DB_NAME: &str = "metadata.db";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub id: String,
    pub timestamp: i64,
    pub label: Option<String>,
    pub description: Option<String>,
    pub config_files: Vec<String>,
    pub checksum: String,
    pub size_bytes: u64,
}

impl ConfigSnapshot {
    pub fn new(
        label: Option<String>,
        description: Option<String>,
        config_files: Vec<String>,
        checksum: String,
        size_bytes: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().timestamp(),
            label,
            description,
            config_files,
            checksum,
            size_bytes,
        }
    }

    pub fn datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.timestamp, 0)
            .unwrap_or_else(|| Utc::now())
    }

    pub fn formatted_time(&self) -> String {
        self.datetime().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFilter {
    pub label: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<usize>,
}

impl Default for SnapshotFilter {
    fn default() -> Self {
        Self {
            label: None,
            since: None,
            until: None,
            limit: Some(50),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSnapshotOptions {
    pub label: Option<String>,
    pub description: Option<String>,
    pub config_dir: Option<String>,
    pub include_patterns: Option<Vec<String>>,
}

impl Default for CreateSnapshotOptions {
    fn default() -> Self {
        Self {
            label: None,
            description: None,
            config_dir: None,
            include_patterns: None,
        }
    }
}

pub struct SnapshotManager {
    store: SnapshotStore,
    snapshots_dir: PathBuf,
    config_dir: PathBuf,
}

impl SnapshotManager {
    pub fn new(zeroclaw_dir: &Path, config_dir: Option<&Path>) -> Result<Self> {
        let snapshots_dir = zeroclaw_dir.join(SNAPSHOT_DIR_NAME);
        std::fs::create_dir_all(&snapshots_dir)
            .with_context(|| "Failed to create snapshots directory")?;

        let db_path = snapshots_dir.join(METADATA_DB_NAME);
        let store = SnapshotStore::new(&db_path)?;

        let config_dir = config_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_DIR));

        Ok(Self {
            store,
            snapshots_dir,
            config_dir,
        })
    }

    pub fn create_snapshot(&self, options: CreateSnapshotOptions) -> Result<ConfigSnapshot> {
        let config_dir = options
            .config_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.config_dir.clone());

        if !config_dir.exists() {
            bail!("Config directory does not exist: {:?}", config_dir);
        }

        let config_files = self.collect_config_files(&config_dir, options.include_patterns.as_deref())?;
        
        if config_files.is_empty() {
            bail!("No configuration files found in {:?}", config_dir);
        }

        let (archive_data, checksum) = self.create_archive(&config_dir, &config_files)?;
        let size_bytes = archive_data.len() as u64;

        let snapshot = ConfigSnapshot::new(
            options.label,
            options.description,
            config_files,
            checksum,
            size_bytes,
        );

        let archive_path = self.snapshot_archive_path(&snapshot.id);
        let mut file = File::create(&archive_path)
            .with_context(|| format!("Failed to create archive file: {:?}", archive_path))?;
        file.write_all(&archive_data)
            .with_context(|| "Failed to write archive data")?;

        self.store.insert_snapshot(&snapshot)?;

        Ok(snapshot)
    }

    pub fn list_snapshots(&self, filter: SnapshotFilter) -> Result<Vec<ConfigSnapshot>> {
        self.store.list_snapshots(filter)
    }

    pub fn get_snapshot(&self, id: &str) -> Result<Option<ConfigSnapshot>> {
        self.store.get_snapshot(id)
    }

    pub fn delete_snapshot(&self, id: &str) -> Result<bool> {
        let snapshot = match self.store.get_snapshot(id)? {
            Some(s) => s,
            None => return Ok(false),
        };

        let archive_path = self.snapshot_archive_path(&snapshot.id);
        if archive_path.exists() {
            std::fs::remove_file(&archive_path)
                .with_context(|| format!("Failed to remove archive: {:?}", archive_path))?;
        }

        self.store.delete_snapshot(id)?;
        Ok(true)
    }

    pub fn snapshot_archive_path(&self, id: &str) -> PathBuf {
        self.snapshots_dir.join(format!("{}.tar.gz", id))
    }

    fn collect_config_files(
        &self,
        config_dir: &Path,
        include_patterns: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let mut seen = HashSet::new();

        let entries = std::fs::read_dir(config_dir)
            .with_context(|| format!("Failed to read config directory: {:?}", config_dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();

            if let Some(patterns) = include_patterns {
                let matches = patterns.iter().any(|p| {
                    if p.contains('*') {
                        glob_match(p, &file_name)
                    } else {
                        &file_name == p
                    }
                });
                if !matches {
                    continue;
                }
            }

            if seen.insert(file_name.clone()) {
                files.push(file_name);
            }
        }

        files.sort();
        Ok(files)
    }

    fn create_archive(
        &self,
        config_dir: &Path,
        config_files: &[String],
    ) -> Result<(Vec<u8>, String)> {
        let mut hasher = Sha256::new();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        {
            let mut tar_builder = tar::Builder::new(&mut encoder);

            for file_name in config_files {
                let file_path = config_dir.join(file_name);
                if !file_path.exists() {
                    continue;
                }

                let mut file = File::open(&file_path)
                    .with_context(|| format!("Failed to open file: {:?}", file_path))?;
                
                let mut content = Vec::new();
                file.read_to_end(&mut content)
                    .with_context(|| format!("Failed to read file: {:?}", file_path))?;

                hasher.update(&content);

                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();

                tar_builder
                    .append_data(&mut header, file_name, content.as_slice())
                    .with_context(|| format!("Failed to append file to archive: {}", file_name))?;
            }

            tar_builder.finish().context("Failed to finalize tar archive")?;
        }
        let compressed = encoder.finish().context("Failed to finalize gzip compression")?;

        let checksum = format!("{:x}", hasher.finalize());
        Ok((compressed, checksum))
    }

    pub fn restore_snapshot(&self, id: &str, target_dir: Option<&Path>) -> Result<Vec<String>> {
        let snapshot = self
            .store
            .get_snapshot(id)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", id))?;

        let archive_path = self.snapshot_archive_path(&snapshot.id);
        if !archive_path.exists() {
            bail!("Snapshot archive not found: {:?}", archive_path);
        }

        let target = target_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.config_dir.clone());

        if !target.exists() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("Failed to create target directory: {:?}", target))?;
        }

        let file = File::open(&archive_path)
            .with_context(|| format!("Failed to open archive: {:?}", archive_path))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        let mut restored = Vec::new();
        for entry in archive.entries().context("Failed to read archive entries")? {
            let mut entry = entry?;
            let path = entry.path()?.to_string_lossy().to_string();
            
            let dest_path = target.join(&path);
            entry
                .unpack(&dest_path)
                .with_context(|| format!("Failed to unpack: {}", path))?;
            
            restored.push(path);
        }

        Ok(restored)
    }

    pub fn verify_snapshot(&self, id: &str) -> Result<bool> {
        let snapshot = self
            .store
            .get_snapshot(id)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", id))?;

        let archive_path = self.snapshot_archive_path(&snapshot.id);
        if !archive_path.exists() {
            return Ok(false);
        }

        let file = File::open(&archive_path)
            .with_context(|| format!("Failed to open archive: {:?}", archive_path))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        let mut hasher = Sha256::new();
        for entry in archive.entries().context("Failed to read archive entries")? {
            let mut entry = entry?;
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            hasher.update(&content);
        }

        let computed_checksum = format!("{:x}", hasher.finalize());
        Ok(computed_checksum == snapshot.checksum)
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    
    fn match_helper(pattern: &[char], text: &[char]) -> bool {
        match (pattern.first(), text.first()) {
            (None, None) => true,
            (Some('*'), _) => {
                match_helper(&pattern[1..], text) 
                    || (!text.is_empty() && match_helper(pattern, &text[1..]))
            }
            (Some(p), Some(t)) if *p == *t => {
                match_helper(&pattern[1..], &text[1..])
            }
            (Some(_), None) => pattern.iter().all(|&c| c == '*'),
            _ => false,
        }
    }
    
    match_helper(&pattern_chars, &text_chars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, TempDir, SnapshotManager) {
        let zeroclaw_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();

        let config_path = config_dir.path();
        std::fs::write(config_path.join("network"), "config interface 'lan'\n").unwrap();
        std::fs::write(config_path.join("wireless"), "config wifi-iface\n").unwrap();
        std::fs::write(config_path.join("firewall"), "config defaults\n").unwrap();

        let manager = SnapshotManager::new(zeroclaw_dir.path(), Some(config_path)).unwrap();
        (zeroclaw_dir, config_dir, manager)
    }

    #[test]
    fn creates_snapshot_successfully() {
        let (_zeroclaw_dir, _config_dir, manager) = setup_test_env();

        let options = CreateSnapshotOptions {
            label: Some("test-snapshot".to_string()),
            description: Some("Test snapshot".to_string()),
            ..Default::default()
        };

        let snapshot = manager.create_snapshot(options).unwrap();

        assert!(!snapshot.id.is_empty());
        assert!(snapshot.label.is_some());
        assert_eq!(snapshot.label.as_deref(), Some("test-snapshot"));
        assert!(!snapshot.config_files.is_empty());
        assert!(!snapshot.checksum.is_empty());
        assert!(snapshot.size_bytes > 0);
    }

    #[test]
    fn lists_snapshots_with_filter() {
        let (_zeroclaw_dir, _config_dir, manager) = setup_test_env();

        manager.create_snapshot(CreateSnapshotOptions {
            label: Some("label-a".to_string()),
            ..Default::default()
        }).unwrap();

        manager.create_snapshot(CreateSnapshotOptions {
            label: Some("label-b".to_string()),
            ..Default::default()
        }).unwrap();

        let all = manager.list_snapshots(SnapshotFilter::default()).unwrap();
        assert_eq!(all.len(), 2);

        let filtered = manager.list_snapshots(SnapshotFilter {
            label: Some("label-a".to_string()),
            ..Default::default()
        }).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, Some("label-a".to_string()));
    }

    #[test]
    fn deletes_snapshot() {
        let (_zeroclaw_dir, _config_dir, manager) = setup_test_env();

        let snapshot = manager.create_snapshot(CreateSnapshotOptions::default()).unwrap();
        let id = snapshot.id.clone();

        let archive_path = manager.snapshot_archive_path(&id);
        assert!(archive_path.exists());

        let deleted = manager.delete_snapshot(&id).unwrap();
        assert!(deleted);

        let not_found = manager.get_snapshot(&id).unwrap();
        assert!(not_found.is_none());
        assert!(!archive_path.exists());
    }

    #[test]
    fn verifies_snapshot_checksum() {
        let (_zeroclaw_dir, _config_dir, manager) = setup_test_env();

        let snapshot = manager.create_snapshot(CreateSnapshotOptions::default()).unwrap();
        let valid = manager.verify_snapshot(&snapshot.id).unwrap();
        assert!(valid);
    }

    #[test]
    fn restores_snapshot() {
        let (_zeroclaw_dir, config_dir, manager) = setup_test_env();

        let snapshot = manager.create_snapshot(CreateSnapshotOptions::default()).unwrap();

        std::fs::remove_file(config_dir.path().join("network")).unwrap();
        assert!(!config_dir.path().join("network").exists());

        let restored = manager.restore_snapshot(&snapshot.id, None).unwrap();
        assert!(!restored.is_empty());
        assert!(config_dir.path().join("network").exists());
    }

    #[test]
    fn filters_by_include_patterns() {
        let (_zeroclaw_dir, config_dir, manager) = setup_test_env();

        let snapshot = manager.create_snapshot(CreateSnapshotOptions {
            include_patterns: Some(vec!["network".to_string()]),
            ..Default::default()
        }).unwrap();

        assert_eq!(snapshot.config_files.len(), 1);
        assert_eq!(snapshot.config_files[0], "network");
    }

    #[test]
    fn glob_match_works_correctly() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("net*", "network"));
        assert!(glob_match("*work", "network"));
        assert!(glob_match("net*work", "network"));
        assert!(!glob_match("net*", "firewall"));
    }

    #[test]
    fn snapshot_datetime_formatting() {
        let snapshot = ConfigSnapshot::new(
            None,
            None,
            vec!["test".to_string()],
            "abc123".to_string(),
            100,
        );
        
        let formatted = snapshot.formatted_time();
        assert!(formatted.contains("UTC"));
    }
}
