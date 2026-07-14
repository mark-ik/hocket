//! Host-local Strophe identity, persisted outside portable projects.

use std::path::{Path, PathBuf};

use personae::{
    DerivedKeyAttestation, Ed25519Keypair, Ed25519PublicKey, IdentityError, IdentityProvider,
    SealedIdentityProvider, SealedRecordStorage, load_or_create_auto_unlock_root,
};

const IDENTITY_RECORD: &str = "strophe/local-identity.json";

/// A durable host identity whose secret is held in memory only while Strophe runs.
pub struct LocalIdentity {
    provider: SealedIdentityProvider,
}

impl LocalIdentity {
    /// Load or create the identity under Strophe's platform data directory.
    pub fn open_default() -> Result<Self, IdentityError> {
        let data_root = default_data_root()?;
        let unlock_path = data_root.join("personae/auto-unlock-root.json");
        let root_key = load_or_create_auto_unlock_root(unlock_path)?.ok_or_else(|| {
            IdentityError::Backend(
                "OS-protected automatic identity unlock is unavailable on this platform"
                    .to_string(),
            )
        })?;
        Self::open_with_root(&data_root.join("personae/records"), root_key)
    }

    fn open_with_root(records_root: &Path, root_key: [u8; 32]) -> Result<Self, IdentityError> {
        let records = SealedRecordStorage::open_with_key(records_root, root_key);
        let provider = SealedIdentityProvider::load_or_create(&records, IDENTITY_RECORD)?;
        Ok(Self { provider })
    }

    /// Short display fingerprint of the public key. This is not an address.
    pub fn fingerprint(&self) -> String {
        self.provider
            .master_public_key()
            .to_bytes()
            .iter()
            .take(6)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

impl IdentityProvider for LocalIdentity {
    fn master_public_key(&self) -> Ed25519PublicKey {
        self.provider.master_public_key()
    }

    fn derive_keypair(&self, salt: &[u8]) -> Result<Ed25519Keypair, IdentityError> {
        self.provider.derive_keypair(salt)
    }

    fn attest_derived_key(&self, salt: &[u8]) -> Result<DerivedKeyAttestation, IdentityError> {
        self.provider.attest_derived_key(salt)
    }
}

fn default_data_root() -> Result<PathBuf, IdentityError> {
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(root).join("Strophe"));
    }
    if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(root).join("strophe"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/share/strophe"));
    }
    Err(IdentityError::Backend(
        "could not determine Strophe's local data directory".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use personae::IdentityProvider;

    use super::*;

    #[test]
    fn sealed_identity_is_stable_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let first = LocalIdentity::open_with_root(dir.path(), [0x45; 32]).unwrap();
        let first_public = first.master_public_key();
        drop(first);

        let second = LocalIdentity::open_with_root(dir.path(), [0x45; 32]).unwrap();
        assert_eq!(second.master_public_key(), first_public);
    }

    #[test]
    fn wrong_record_root_cannot_open_identity() {
        let dir = tempfile::tempdir().unwrap();
        LocalIdentity::open_with_root(dir.path(), [0x45; 32]).unwrap();

        let error = LocalIdentity::open_with_root(dir.path(), [0x46; 32])
            .err()
            .expect("wrong root should fail");
        assert!(error.to_string().contains("decrypt sealed record"));
    }
}
