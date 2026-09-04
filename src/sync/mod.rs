//! Push and pull encrypted profiles to a remote store.
//!
//! Profiles are already encrypted on disk, so sync moves raw bytes. Conflict
//! detection compares SHA-256 hashes of the ciphertext on each side against
//! the hash recorded the last time the two sides agreed.

use sha2::{Digest, Sha256};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A configured remote. Doubles as the on-disk config shape in `sync.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Remote {
    S3 {
        bucket: String,
        prefix: String,
        region: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
    },
    GoogleDrive {
        client_id: String,
        client_secret: String,
        refresh_token: String,
        folder_id: String,
    },
    Dir {
        path: PathBuf,
    },
}

/// Contents of `~/.envio/sync.toml`.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default)]
    pub remotes: BTreeMap<String, Remote>,
}

impl SyncConfig {
    /// Read the config. A missing file is an empty config.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        toml::from_str(&text).map_err(|e| Error::Deserialization(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string(self).map_err(|e| Error::Serialization(e.to_string()))?;
        write_private(path, text.as_bytes())
    }

    /// Pick a remote: the explicit `name`, else `default`, else the only
    /// one configured. Errors list the available names.
    pub fn select(&self, name: Option<&str>) -> Result<(&str, &Remote)> {
        let name = match name.or(self.default.as_deref()) {
            Some(n) => n,
            None if self.remotes.len() == 1 => self.remotes.keys().next().unwrap(),
            None if self.remotes.is_empty() => {
                return Err(Error::Sync(
                    "no remotes configured, run `envio sync remote add <NAME>`".into(),
                ))
            }
            None => {
                let names: Vec<&str> = self.remotes.keys().map(String::as_str).collect();
                return Err(Error::Sync(format!(
                    "several remotes configured, pass --remote <NAME>: {}",
                    names.join(", ")
                )));
            }
        };
        self.remotes
            .get_key_value(name)
            .map(|(k, v)| (k.as_str(), v))
            .ok_or_else(|| Error::Sync(format!("remote `{}` not found in sync.toml", name)))
    }
}

/// Contents of `~/.envio/sync-state.toml`: remote name -> profile name ->
/// SHA-256 of the ciphertext at the last successful sync.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncState(BTreeMap<String, BTreeMap<String, String>>);

impl SyncState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        toml::from_str(&text).map_err(|e| Error::Deserialization(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string(self).map_err(|e| Error::Serialization(e.to_string()))?;
        write_private(path, text.as_bytes())
    }

    pub fn get(&self, remote: &str, profile: &str) -> Option<&str> {
        self.0.get(remote)?.get(profile).map(String::as_str)
    }

    pub fn set(&mut self, remote: &str, profile: &str, sha256: String) {
        self.0
            .entry(remote.to_string())
            .or_default()
            .insert(profile.to_string(), sha256);
    }
}

/// Write `bytes` to `path`, creating it with mode 0600 on Unix.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)?.write_all(bytes)?;
    Ok(())
}

/// Relationship between the local copy, the remote copy, and the last
/// synced hash of one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    UpToDate,
    LocalAhead,
    RemoteAhead,
    Conflict,
    OnlyLocal,
    OnlyRemote,
}

/// Decide the sync status from three optional hashes.
///
/// `local`/`remote` are `None` when that side has no copy. `last` is the hash
/// recorded after the previous successful sync, or `None` if never synced.
/// Callers must not pass `(None, None, _)`; it is reported as `UpToDate`.
pub fn decide(local: Option<&str>, remote: Option<&str>, last: Option<&str>) -> Status {
    match (local, remote) {
        (None, None) => Status::UpToDate,
        (Some(_), None) => Status::OnlyLocal,
        (None, Some(_)) => Status::OnlyRemote,
        (Some(l), Some(r)) if l == r => Status::UpToDate,
        (Some(l), Some(r)) => {
            let local_changed = last != Some(l);
            let remote_changed = last != Some(r);
            match (local_changed, remote_changed) {
                (true, false) => Status::LocalAhead,
                (false, true) => Status::RemoteAhead,
                // (true, true) is a real conflict; (false, false) cannot
                // happen because l != r here.
                _ => Status::Conflict,
            }
        }
    }
}

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

#[cfg(test)]
mod decide_tests {
    use super::*;

    const A: Option<&str> = Some("aaa");
    const B: Option<&str> = Some("bbb");
    const C: Option<&str> = Some("ccc");

    #[test]
    fn same_same_is_up_to_date() {
        assert_eq!(decide(A, A, A), Status::UpToDate);
    }

    #[test]
    fn local_changed_is_local_ahead() {
        assert_eq!(decide(B, A, A), Status::LocalAhead);
    }

    #[test]
    fn remote_changed_is_remote_ahead() {
        assert_eq!(decide(A, B, A), Status::RemoteAhead);
    }

    #[test]
    fn both_changed_is_conflict() {
        assert_eq!(decide(B, C, A), Status::Conflict);
    }

    #[test]
    fn only_local() {
        assert_eq!(decide(A, None, None), Status::OnlyLocal);
        assert_eq!(decide(A, None, A), Status::OnlyLocal);
    }

    #[test]
    fn only_remote() {
        assert_eq!(decide(None, A, None), Status::OnlyRemote);
        assert_eq!(decide(None, A, A), Status::OnlyRemote);
    }

    #[test]
    fn equal_sides_win_regardless_of_last() {
        assert_eq!(decide(B, B, A), Status::UpToDate);
        assert_eq!(decide(B, B, None), Status::UpToDate);
    }

    #[test]
    fn never_synced_but_both_exist_and_differ_is_conflict() {
        assert_eq!(decide(A, B, None), Status::Conflict);
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("envio-sync-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn three_remotes() -> SyncConfig {
        let mut remotes = BTreeMap::new();
        remotes.insert(
            "work".into(),
            Remote::S3 {
                bucket: "my-secrets".into(),
                prefix: "envio".into(),
                region: "eu-central-1".into(),
                endpoint: None,
            },
        );
        remotes.insert(
            "personal".into(),
            Remote::GoogleDrive {
                client_id: "id".into(),
                client_secret: "secret".into(),
                refresh_token: "rt".into(),
                folder_id: "fid".into(),
            },
        );
        remotes.insert(
            "nas".into(),
            Remote::Dir {
                path: PathBuf::from("/mnt/nas/envio"),
            },
        );
        SyncConfig {
            default: Some("work".into()),
            remotes,
        }
    }

    #[test]
    fn config_round_trips_through_toml() {
        let dir = tmp("config-rt");
        let path = dir.join("sync.toml");
        let cfg = three_remotes();
        cfg.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[remotes.work]"), "{}", text);
        assert!(text.contains("type = \"s3\""), "{}", text);
        assert!(text.contains("type = \"google_drive\""), "{}", text);
        assert!(text.contains("type = \"dir\""), "{}", text);
        let back = SyncConfig::load(&path).unwrap();
        assert_eq!(back, cfg);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_config_is_empty() {
        let dir = tmp("config-missing");
        let cfg = SyncConfig::load(&dir.join("nope.toml")).unwrap();
        assert!(cfg.remotes.is_empty());
        assert_eq!(cfg.default, None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn select_prefers_explicit_then_default_then_only() {
        let cfg = three_remotes();
        assert_eq!(cfg.select(Some("nas")).unwrap().0, "nas");
        assert_eq!(cfg.select(None).unwrap().0, "work");
        assert!(cfg.select(Some("missing")).is_err());

        let mut one_remotes = BTreeMap::new();
        one_remotes.insert("only".into(), Remote::Dir { path: "/x".into() });
        let one = SyncConfig {
            default: None,
            remotes: one_remotes,
        };
        assert_eq!(one.select(None).unwrap().0, "only");

        let two_base = three_remotes();
        let two = SyncConfig {
            default: None,
            ..two_base
        };
        let err = two.select(None).unwrap_err().to_string();
        assert!(err.contains("--remote"), "{}", err);
        assert!(err.contains("nas"), "{}", err);

        assert!(SyncConfig::default().select(None).is_err());
    }

    #[test]
    fn state_round_trips_and_defaults_to_none() {
        let dir = tmp("state-rt");
        let path = dir.join("sync-state.toml");
        let mut st = SyncState::load(&path).unwrap();
        assert_eq!(st.get("work", "app"), None);
        st.set("work", "app", "abc".into());
        st.set("work", "with space", "def".into());
        st.save(&path).unwrap();
        let back = SyncState::load(&path).unwrap();
        assert_eq!(back.get("work", "app"), Some("abc"));
        assert_eq!(back.get("work", "with space"), Some("def"));
        assert_eq!(back.get("other", "app"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("perms");
        let path = dir.join("sync.toml");
        three_remotes().save(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
