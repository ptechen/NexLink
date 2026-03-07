use anyhow::{Context, Result};
use libp2p::identity::Keypair;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use tracing::{info, warn, error, debug};

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    keypair_bytes: Vec<u8>,
}

pub struct NodeIdentity {
    keypair: Keypair,
    peer_id: PeerId,
}

impl NodeIdentity {
    pub fn generate() -> Self {
        let keypair = Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();
        info!(%peer_id, "Generated new identity");
        Self { keypair, peer_id }
    }

    /// Validates keypair bytes for corruption before attempting to decode
    fn validate_keypair_bytes(bytes: &[u8]) -> Result<()> {
        // Check for obvious corruption indicators
        if bytes.len() > 1024 {
            return Err(anyhow::anyhow!("Invalid multihash size {}", bytes.len()));
        }

        // Basic protobuf header validation for Ed25519 keypair
        // Ed25519 protobuf header should start with expected magic bytes
        if bytes.len() >= 2 {
            // The first byte should be a valid protobuf varint field descriptor
            // For Ed25519 keys, field number 1 with wire type 2 (length-delimited) gives us 0x0a
            // For RSA keys or other key types, different values are possible
            // We should be more permissive about this check to avoid false warnings
            let expected_headers = [0x0a, 0x12, 0x08, 0x1a]; // Common valid protobuf headers for key types
            if !expected_headers.contains(&bytes[0]) {
                // Log at debug level instead of warn to reduce noise
                debug!("Unexpected protobuf header detected - this may be normal for different key types");
            }
        }

        Ok(())
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let stored = StoredIdentity {
            keypair_bytes: self
                .keypair
                .to_protobuf_encoding()
                .context("Failed to encode keypair")?,
        };
        let json = serde_json::to_string_pretty(&stored)?;

        // Atomic write operation to prevent corruption during writes
        let temp_path = path.with_extension(".tmp");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&temp_path, json)?;
        fs::rename(temp_path, path)?; // Atomic on most filesystems

        info!(?path, "Saved identity to file");
        Ok(())
    }

    /// Load identity from file with basic validation
    fn load_from_file_validated(path: &Path) -> Result<Self> {
        let json = fs::read_to_string(path).context("Failed to read identity file")?;
        let stored: StoredIdentity = serde_json::from_str(&json)?;

        // Validate the keypair bytes before attempting to decode
        Self::validate_keypair_bytes(&stored.keypair_bytes)
            .context("Key validation failed")?;

        let keypair = Keypair::from_protobuf_encoding(&stored.keypair_bytes)
            .context("Failed to parse: invalid multihash")?;

        // Additional validation: ensure the decoded keypair produces a valid PeerId
        let peer_id = keypair.public().to_peer_id();

        // Double-check that we have a reasonable PeerId (valid multihash)
        let peer_id_str = peer_id.to_string();
        if peer_id_str.len() < 10 || peer_id_str.len() > 100 {
            return Err(anyhow::anyhow!("Invalid PeerId format"));
        }

        info!(%peer_id, ?path, "Loaded validated identity from file");
        Ok(Self { keypair, peer_id })
    }

    /// Backs up a corrupted identity file for diagnostics
    fn backup_corrupted_file(path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let backup_path = path.with_extension("corrupted.json");
        let mut counter = 0;
        let mut final_backup_path = backup_path.clone();

        // Handle potential naming conflicts
        while final_backup_path.exists() {
            counter += 1;
            final_backup_path = path.with_extension(format!("corrupted_{}.json", counter));
            if counter > 10 { // Prevent infinite loop
                break;
            }
        }

        if fs::copy(path, &final_backup_path).is_ok() {
            info!(?final_backup_path, "Backed up corrupted identity file");
        }

        Ok(())
    }

    /// Load identity from file with error recovery
    fn load_from_file_with_recovery(path: &Path) -> Result<Self> {
        match Self::load_from_file_validated(path) {
            Ok(identity) => Ok(identity),
            Err(original_error) => {
                warn!(?path, "Failed to load identity, attempting recovery: {}", original_error);

                // Backup the corrupted file before attempting to regenerate
                if let Err(backup_err) = Self::backup_corrupted_file(path) {
                    warn!("Failed to backup corrupted file: {}", backup_err);
                }

                // Regenerate identity
                warn!("Regenerating identity due to corruption");
                let identity = Self::generate();

                // Attempt to save the new identity to file
                if let Err(save_err) = identity.save_to_file(path) {
                    error!("Failed to save regenerated identity: {}", save_err);
                    return Err(save_err.context("Could not save regenerated identity"));
                }

                Ok(identity)
            }
        }
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        Self::load_from_file_validated(path)
    }

    pub fn load_or_generate_with_recovery(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load_from_file_with_recovery(path)
        } else {
            let identity = Self::generate();
            identity.save_to_file(path)?;
            Ok(identity)
        }
    }

    /// Legacy function maintained for backward compatibility
    pub fn load_or_generate(path: &Path) -> Result<Self> {
        Self::load_or_generate_with_recovery(path)
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_new_identity() {
        let id = NodeIdentity::generate();
        assert!(!id.peer_id().to_string().is_empty());
    }

    #[test]
    fn test_save_and_load_identity() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.json");

        let id1 = NodeIdentity::generate();
        id1.save_to_file(&path).unwrap();

        let id2 = NodeIdentity::load_from_file(&path).unwrap();
        assert_eq!(id1.peer_id(), id2.peer_id());
    }

    #[test]
    fn test_load_or_generate_creates_new() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.json");

        let id = NodeIdentity::load_or_generate(&path).unwrap();
        assert!(path.exists());
        assert!(!id.peer_id().to_string().is_empty());
    }

    #[test]
    fn test_load_or_generate_loads_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.json");

        let id1 = NodeIdentity::load_or_generate(&path).unwrap();
        let id2 = NodeIdentity::load_or_generate(&path).unwrap();
        assert_eq!(id1.peer_id(), id2.peer_id());
    }

    #[test]
    fn test_load_from_file_with_recovery_handles_corruption() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.json");

        // Create a corrupted identity file
        std::fs::write(&path, r#"{"keypair_bytes": [255, 255, 255, 255]}"#).unwrap();

        // Test that recovery generates a new identity instead of crashing
        let result = NodeIdentity::load_from_file_with_recovery(&path);
        assert!(result.is_ok());

        // The old file should have been backed up
        let backup_path = path.with_extension("corrupted.json");
        assert!(backup_path.exists());

        // A new identity should be created and saved
        let recovered_id = result.unwrap();
        assert!(!recovered_id.peer_id().to_string().is_empty());

        // The new file should be valid
        let reloaded_id = NodeIdentity::load_from_file(&path).unwrap();
        assert_eq!(recovered_id.peer_id(), reloaded_id.peer_id());
    }

    #[test]
    fn test_validate_keypair_bytes_detection() {
        // Test that oversized keypair is rejected
        let oversized_bytes = vec![0u8; 2048]; // Much larger than 1024 byte limit
        assert!(NodeIdentity::validate_keypair_bytes(&oversized_bytes).is_err());

        // Test that valid-sized keypair passes validation
        let valid_bytes = vec![0x0a, 0x20]; // Minimal valid protobuf-like header
        assert!(NodeIdentity::validate_keypair_bytes(&valid_bytes).is_ok());
    }
}
