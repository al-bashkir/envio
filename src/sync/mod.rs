//! Push and pull encrypted profiles to a remote store.
//!
//! Profiles are already encrypted on disk, so sync moves raw bytes. Conflict
//! detection compares SHA-256 hashes of the ciphertext on each side against
//! the hash recorded the last time the two sides agreed.

use sha2::{Digest, Sha256};

pub mod gdrive;
pub mod s3;

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

/// One profile as seen on a remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub sha256: String,
}

impl Remote {
    /// Every `<name>.env` object under this remote's prefix, sorted by name.
    pub fn list(&self) -> Result<Vec<RemoteEntry>> {
        match self {
            Remote::Dir { path } => dir_list(path),
            Remote::S3 {
                bucket,
                prefix,
                region,
                endpoint,
            } => s3::S3::new(bucket, prefix, region, endpoint.as_deref())?.list(),
            Remote::GoogleDrive {
                client_id,
                client_secret,
                refresh_token,
                folder_id,
            } => gdrive::Drive::new(client_id, client_secret, refresh_token, folder_id)?.list(),
        }
    }

    /// Raw bytes of `<name>.env` on the remote.
    pub fn get(&self, name: &str) -> Result<Vec<u8>> {
        check_name(name)?;
        match self {
            Remote::Dir { path } => Ok(std::fs::read(path.join(format!("{}.env", name)))?),
            Remote::S3 {
                bucket,
                prefix,
                region,
                endpoint,
            } => s3::S3::new(bucket, prefix, region, endpoint.as_deref())?.get(name),
            Remote::GoogleDrive {
                client_id,
                client_secret,
                refresh_token,
                folder_id,
            } => gdrive::Drive::new(client_id, client_secret, refresh_token, folder_id)?.get(name),
        }
    }

    /// Create or overwrite `<name>.env` on the remote.
    pub fn put(&self, name: &str, bytes: &[u8]) -> Result<()> {
        check_name(name)?;
        match self {
            Remote::Dir { path } => write_atomic(&path.join(format!("{}.env", name)), bytes),
            Remote::S3 {
                bucket,
                prefix,
                region,
                endpoint,
            } => s3::S3::new(bucket, prefix, region, endpoint.as_deref())?.put(name, bytes),
            Remote::GoogleDrive {
                client_id,
                client_secret,
                refresh_token,
                folder_id,
            } => gdrive::Drive::new(client_id, client_secret, refresh_token, folder_id)?
                .put(name, bytes),
        }
    }
}

fn dir_list(path: &Path) -> Result<Vec<RemoteEntry>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let p = entry?.path();
        let Some(name) = p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".env"))
        else {
            continue;
        };
        if check_name(name).is_err() {
            continue;
        }
        out.push(RemoteEntry {
            name: name.to_string(),
            sha256: sha256_hex(&std::fs::read(&p)?),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Write to a sibling temp file, then rename over `dest`, so a dropped
/// network share or a killed process cannot leave a truncated profile.
pub(crate) fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::Sync(format!("bad path {}", dest.display())))?;
    let tmp = dest.with_file_name(format!(".{}.tmp", file_name));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// Reject names that could escape the profiles directory or collide with
/// temp files. Applied to CLI arguments and to names reported by remotes.
pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == ".."
        || name.starts_with('.')
        || crate::utils::contains_path_separator(name)
    {
        return Err(Error::Sync(format!("invalid profile name `{}`", name)));
    }
    Ok(())
}

/// What happened to one profile during push or pull.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Uploaded,
    Downloaded,
    UpToDate,
    /// Refused because of the given status; `--force` overrides.
    Refused(Status),
    /// The named profile does not exist on the side it would be read from.
    Missing(&'static str),
    /// The backend call failed. Other profiles continue.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub profile: String,
    pub outcome: Outcome,
}

/// Names of every `<name>.env` in the profiles directory, sorted.
///
/// Unlike `dir_list`, this never reads file contents — it only needs names,
/// so a directory (or anything else unreadable as a profile) is skipped
/// rather than aborting the whole listing. `read_dir` itself failing (a
/// missing or unreadable profiles directory) is still a whole-operation
/// failure and propagates.
pub fn local_profile_names(profiles_dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(profiles_dir)? {
        let entry = entry?;
        let is_file = match entry.file_type() {
            Ok(t) => t.is_file(),
            Err(_) => entry.path().is_file(),
        };
        if !is_file {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str().and_then(|n| n.strip_suffix(".env")) else {
            continue;
        };
        if check_name(name).is_err() {
            continue;
        }
        out.push(name.to_string());
    }
    out.sort();
    Ok(out)
}

/// One remote bound to one local profiles directory and state file.
pub struct Sync<'a> {
    pub remote_name: &'a str,
    pub remote: &'a Remote,
    pub profiles_dir: &'a Path,
    pub state_path: &'a Path,
}

impl Sync<'_> {
    fn local_path(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(format!("{}.env", name))
    }

    fn local_bytes(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match std::fs::read(self.local_path(name)) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn remote_hashes(&self) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        for e in self.remote.list()? {
            // Skip anything a pull could not safely write, e.g. `..` or a
            // dot-file someone dropped in the bucket by hand.
            if check_name(&e.name).is_ok() {
                out.insert(e.name, e.sha256);
            }
        }
        Ok(out)
    }

    fn validate(names: &[String]) -> Result<()> {
        names.iter().try_for_each(|n| check_name(n))
    }

    /// Upload `profiles` (or every local profile if empty).
    pub fn push(&self, profiles: &[String], force: bool) -> Result<Vec<Report>> {
        Self::validate(profiles)?;
        let mut state = SyncState::load(self.state_path)?;
        let remote = self.remote_hashes()?;
        let names = if profiles.is_empty() {
            local_profile_names(self.profiles_dir)?
        } else {
            profiles.to_vec()
        };

        let mut reports = Vec::with_capacity(names.len());
        for name in names {
            let local = match self.local_bytes(&name) {
                Ok(local) => local,
                Err(e) => {
                    reports.push(Report {
                        profile: name,
                        outcome: Outcome::Failed(e.to_string()),
                    });
                    continue;
                }
            };
            let local_hash = local.as_deref().map(sha256_hex);
            let remote_hash = remote.get(&name).map(String::as_str);
            let status = decide(
                local_hash.as_deref(),
                remote_hash,
                state.get(self.remote_name, &name),
            );

            let outcome = match (status, &local) {
                (_, None) => Outcome::Missing("no such local profile"),
                (Status::UpToDate, Some(_)) => {
                    state.set(self.remote_name, &name, local_hash.clone().unwrap());
                    Outcome::UpToDate
                }
                (Status::OnlyLocal | Status::LocalAhead, Some(bytes)) => {
                    self.upload(&mut state, &name, bytes)
                }
                (Status::RemoteAhead | Status::Conflict, Some(bytes)) if force => {
                    self.upload(&mut state, &name, bytes)
                }
                (Status::RemoteAhead | Status::Conflict, Some(_)) => Outcome::Refused(status),
                (Status::OnlyRemote, Some(_)) => unreachable!("local is Some"),
            };
            reports.push(Report {
                profile: name,
                outcome,
            });
        }
        state.save(self.state_path)?;
        Ok(reports)
    }

    fn upload(&self, state: &mut SyncState, name: &str, bytes: &[u8]) -> Outcome {
        match self.remote.put(name, bytes) {
            Ok(()) => {
                state.set(self.remote_name, name, sha256_hex(bytes));
                Outcome::Uploaded
            }
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    /// Download `profiles` (or every remote profile if empty).
    pub fn pull(&self, profiles: &[String], force: bool) -> Result<Vec<Report>> {
        Self::validate(profiles)?;
        let mut state = SyncState::load(self.state_path)?;
        let remote = self.remote_hashes()?;
        let names: Vec<String> = if profiles.is_empty() {
            remote.keys().cloned().collect()
        } else {
            profiles.to_vec()
        };

        let mut reports = Vec::with_capacity(names.len());
        for name in names {
            let local = match self.local_bytes(&name) {
                Ok(local) => local,
                Err(e) => {
                    reports.push(Report {
                        profile: name,
                        outcome: Outcome::Failed(e.to_string()),
                    });
                    continue;
                }
            };
            let local_hash = local.map(|b| sha256_hex(&b));
            let remote_hash = remote.get(&name).map(String::as_str);
            let status = decide(
                local_hash.as_deref(),
                remote_hash,
                state.get(self.remote_name, &name),
            );

            let outcome = match (status, remote_hash) {
                (_, None) => Outcome::Missing("not on remote"),
                (Status::UpToDate, Some(h)) => {
                    state.set(self.remote_name, &name, h.to_string());
                    Outcome::UpToDate
                }
                (Status::OnlyRemote | Status::RemoteAhead, Some(_)) => {
                    self.download(&mut state, &name)
                }
                (Status::LocalAhead | Status::Conflict, Some(_)) if force => {
                    self.download(&mut state, &name)
                }
                (Status::LocalAhead | Status::Conflict, Some(_)) => Outcome::Refused(status),
                (Status::OnlyLocal, Some(_)) => unreachable!("remote is Some"),
            };
            reports.push(Report {
                profile: name,
                outcome,
            });
        }
        state.save(self.state_path)?;
        Ok(reports)
    }

    fn download(&self, state: &mut SyncState, name: &str) -> Outcome {
        let result = self
            .remote
            .get(name)
            .and_then(|bytes| write_atomic(&self.local_path(name), &bytes).map(|_| bytes));
        match result {
            Ok(bytes) => {
                state.set(self.remote_name, name, sha256_hex(&bytes));
                Outcome::Downloaded
            }
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    /// Status of every profile present locally or on the remote, sorted.
    pub fn status(&self) -> Result<Vec<(String, Status)>> {
        let state = SyncState::load(self.state_path)?;
        let remote = self.remote_hashes()?;
        let mut names: Vec<String> = local_profile_names(self.profiles_dir)?;
        names.extend(remote.keys().cloned());
        names.sort();
        names.dedup();

        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let local_hash = self.local_bytes(&name)?.map(|b| sha256_hex(&b));
            let status = decide(
                local_hash.as_deref(),
                remote.get(&name).map(String::as_str),
                state.get(self.remote_name, &name),
            );
            out.push((name, status));
        }
        Ok(out)
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

#[cfg(test)]
mod dir_tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("envio-sync-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dir_put_list_get() {
        let dir = tmp("dir-backend");
        let remote = Remote::Dir { path: dir.clone() };
        remote.put("work", b"cipher-1").unwrap();
        remote.put("staging", b"cipher-2").unwrap();
        std::fs::write(dir.join("notes.txt"), b"ignored").unwrap();

        let entries = remote.list().unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["staging", "work"]);
        assert_eq!(entries[1].sha256, sha256_hex(b"cipher-1"));
        assert_eq!(remote.get("work").unwrap(), b"cipher-1");
        assert!(remote.get("missing").is_err());

        // no temp files left behind
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{:?}", leftovers);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dir_put_overwrites() {
        let dir = tmp("dir-overwrite");
        let remote = Remote::Dir { path: dir.clone() };
        remote.put("work", b"one").unwrap();
        remote.put("work", b"two").unwrap();
        assert_eq!(remote.get("work").unwrap(), b"two");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn check_name_rejects_paths() {
        assert!(check_name("work").is_ok());
        assert!(check_name("my-app_v2").is_ok());
        assert!(check_name("").is_err());
        assert!(check_name("..").is_err());
        assert!(check_name(".hidden").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name("a\\b").is_err());
    }

    #[test]
    fn get_rejects_path_traversal() {
        let dir = tmp("get-traversal");
        let remote = Remote::Dir { path: dir.clone() };
        let err = remote.get("../evil").unwrap_err();
        assert!(err.to_string().contains("invalid profile name"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn put_rejects_path_traversal() {
        let root = tmp("put-traversal-root");
        let remote_dir = root.join("remote");
        std::fs::create_dir(&remote_dir).unwrap();
        let remote = Remote::Dir {
            path: remote_dir.clone(),
        };

        let err = remote.put("../evil", b"payload").unwrap_err();
        assert!(err.to_string().contains("invalid profile name"));

        // Verify no file was created outside the remote dir
        let evil_path = root.join("evil.env");
        assert!(
            !evil_path.exists(),
            "File should not exist at {:?}",
            evil_path
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn put_rejects_absolute_path() {
        let dir = tmp("put-absolute");
        let remote = Remote::Dir { path: dir.clone() };

        // Try to write to an absolute path
        let abs_name = "/tmp/envio-should-not-exist";
        let err = remote.put(abs_name, b"payload").unwrap_err();
        assert!(err.to_string().contains("invalid profile name"));

        // Verify no file was created at the absolute path
        let abs_target = PathBuf::from("/tmp/envio-should-not-exist.env");
        assert!(
            !abs_target.exists(),
            "File should not exist at {:?}",
            abs_target
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        remote: Remote,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("envio-sync-{}-{}", std::process::id(), name));
            let _ = std::fs::remove_dir_all(&root);
            for sub in ["a/profiles", "b/profiles", "remote"] {
                std::fs::create_dir_all(root.join(sub)).unwrap();
            }
            let remote = Remote::Dir {
                path: root.join("remote"),
            };
            Fixture { root, remote }
        }

        fn sync(&self, machine: &str) -> Sync<'_> {
            // Leak the paths: tests only, keeps the borrow simple.
            let profiles_dir: &'static Path =
                Box::leak(self.root.join(machine).join("profiles").into_boxed_path());
            let state_path: &'static Path = Box::leak(
                self.root
                    .join(machine)
                    .join("sync-state.toml")
                    .into_boxed_path(),
            );
            Sync {
                remote_name: "test",
                remote: &self.remote,
                profiles_dir,
                state_path,
            }
        }

        fn write(&self, machine: &str, name: &str, bytes: &[u8]) {
            std::fs::write(
                self.root
                    .join(machine)
                    .join("profiles")
                    .join(format!("{}.env", name)),
                bytes,
            )
            .unwrap();
        }

        fn read(&self, machine: &str, name: &str) -> Vec<u8> {
            std::fs::read(
                self.root
                    .join(machine)
                    .join("profiles")
                    .join(format!("{}.env", name)),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn outcomes(reports: &[Report]) -> Vec<(&str, &Outcome)> {
        reports
            .iter()
            .map(|r| (r.profile.as_str(), &r.outcome))
            .collect()
    }

    #[test]
    fn push_all_then_pull_all_on_second_machine() {
        let f = Fixture::new("roundtrip");
        f.write("a", "work", b"cipher-work");
        f.write("a", "staging", b"cipher-staging");

        let reports = f.sync("a").push(&[], false).unwrap();
        assert_eq!(
            outcomes(&reports),
            vec![
                ("staging", &Outcome::Uploaded),
                ("work", &Outcome::Uploaded)
            ]
        );

        let reports = f.sync("b").pull(&[], false).unwrap();
        assert_eq!(
            outcomes(&reports),
            vec![
                ("staging", &Outcome::Downloaded),
                ("work", &Outcome::Downloaded)
            ]
        );
        assert_eq!(f.read("b", "work"), b"cipher-work");
        assert_eq!(f.read("b", "staging"), b"cipher-staging");

        let state = SyncState::load(&f.root.join("b/sync-state.toml")).unwrap();
        assert_eq!(
            state.get("test", "work"),
            Some(sha256_hex(b"cipher-work").as_str())
        );

        // second push/pull is a no-op
        let reports = f.sync("a").push(&[], false).unwrap();
        assert!(reports.iter().all(|r| r.outcome == Outcome::UpToDate));
        let reports = f.sync("b").pull(&[], false).unwrap();
        assert!(reports.iter().all(|r| r.outcome == Outcome::UpToDate));
    }

    #[test]
    fn remote_ahead_refuses_push_unless_forced() {
        let f = Fixture::new("remote-ahead");
        f.write("a", "work", b"v1");
        f.sync("a").push(&[], false).unwrap();
        // machine b pulls v1, edits, pushes v2
        f.sync("b").pull(&[], false).unwrap();
        f.write("b", "work", b"v2");
        let reports = f.sync("b").push(&[], false).unwrap();
        assert_eq!(reports[0].outcome, Outcome::Uploaded);

        // machine a still has v1 and last-synced v1: remote is ahead
        let reports = f.sync("a").push(&["work".into()], false).unwrap();
        assert_eq!(reports[0].outcome, Outcome::Refused(Status::RemoteAhead));
        assert_eq!(f.remote.get("work").unwrap(), b"v2");

        let reports = f.sync("a").push(&["work".into()], true).unwrap();
        assert_eq!(reports[0].outcome, Outcome::Uploaded);
        assert_eq!(f.remote.get("work").unwrap(), b"v1");
    }

    #[test]
    fn local_ahead_refuses_pull_unless_forced() {
        let f = Fixture::new("local-ahead");
        f.write("a", "work", b"v1");
        f.sync("a").push(&[], false).unwrap();
        f.write("a", "work", b"v1-edited");

        let reports = f.sync("a").pull(&["work".into()], false).unwrap();
        assert_eq!(reports[0].outcome, Outcome::Refused(Status::LocalAhead));
        assert_eq!(f.read("a", "work"), b"v1-edited");

        let reports = f.sync("a").pull(&["work".into()], true).unwrap();
        assert_eq!(reports[0].outcome, Outcome::Downloaded);
        assert_eq!(f.read("a", "work"), b"v1");
    }

    #[test]
    fn conflict_refuses_both_ways() {
        let f = Fixture::new("conflict");
        f.write("a", "work", b"v1");
        f.sync("a").push(&[], false).unwrap();
        f.write("a", "work", b"a-edit");
        f.remote.put("work", b"b-edit").unwrap();

        let push = f.sync("a").push(&[], false).unwrap();
        assert_eq!(push[0].outcome, Outcome::Refused(Status::Conflict));
        let pull = f.sync("a").pull(&[], false).unwrap();
        assert_eq!(pull[0].outcome, Outcome::Refused(Status::Conflict));
    }

    #[test]
    fn named_profile_missing_on_either_side() {
        let f = Fixture::new("missing");
        f.write("a", "work", b"v1");
        let pull = f.sync("a").pull(&["work".into()], false).unwrap();
        assert!(matches!(pull[0].outcome, Outcome::Missing(_)));
        let push = f.sync("a").push(&["ghost".into()], false).unwrap();
        assert!(matches!(push[0].outcome, Outcome::Missing(_)));
        assert!(f.sync("a").push(&["../evil".into()], false).is_err());
    }

    #[test]
    fn status_reports_every_row() {
        let f = Fixture::new("status");
        f.write("a", "same", b"s");
        f.write("a", "local-ahead", b"l1");
        f.write("a", "remote-ahead", b"r1");
        f.write("a", "only-local", b"o");
        f.sync("a").push(&[], false).unwrap();
        f.write("a", "local-ahead", b"l2");
        f.remote.put("remote-ahead", b"r2").unwrap();
        f.remote.put("only-remote", b"x").unwrap();
        // make only-local truly only local again
        std::fs::remove_file(f.root.join("remote/only-local.env")).unwrap();

        let status: BTreeMap<String, Status> = f.sync("a").status().unwrap().into_iter().collect();
        assert_eq!(status["same"], Status::UpToDate);
        assert_eq!(status["local-ahead"], Status::LocalAhead);
        assert_eq!(status["remote-ahead"], Status::RemoteAhead);
        assert_eq!(status["only-local"], Status::OnlyLocal);
        assert_eq!(status["only-remote"], Status::OnlyRemote);
    }

    #[test]
    fn hard_local_read_error_mid_batch_does_not_lose_batch() {
        let f = Fixture::new("hard-local-error");
        f.write("a", "aaa", b"aaa-cipher");
        f.write("a", "zzz", b"zzz-cipher");
        // A directory where a profile file should be: reading it fails with
        // something other than NotFound.
        //
        // Named explicitly (not `push(&[], false)`): an empty-slice push
        // first calls `local_profile_names`, which (via `dir_list`) reads
        // every entry's bytes to compute its remote-listing sha256 and would
        // itself error out on this directory before the per-profile loop —
        // a separate, pre-existing issue in `dir_list`, out of scope here.
        // Naming the profiles explicitly routes straight into the
        // per-profile loop and exercises the `local_bytes` fix directly.
        std::fs::create_dir(f.root.join("a/profiles/mmm.env")).unwrap();

        let reports = f
            .sync("a")
            .push(&["aaa".into(), "mmm".into(), "zzz".into()], false)
            .unwrap();
        let by_name: BTreeMap<&str, &Outcome> = outcomes(&reports).into_iter().collect();
        assert_eq!(by_name.len(), 3);
        assert_eq!(by_name["aaa"], &Outcome::Uploaded);
        assert_eq!(by_name["zzz"], &Outcome::Uploaded);
        assert!(matches!(by_name["mmm"], Outcome::Failed(_)));

        assert_eq!(f.remote.get("aaa").unwrap(), b"aaa-cipher");
        assert_eq!(f.remote.get("zzz").unwrap(), b"zzz-cipher");

        let state = SyncState::load(&f.root.join("a/sync-state.toml")).unwrap();
        assert_eq!(
            state.get("test", "aaa"),
            Some(sha256_hex(b"aaa-cipher").as_str())
        );
        assert_eq!(
            state.get("test", "zzz"),
            Some(sha256_hex(b"zzz-cipher").as_str())
        );
    }

    #[test]
    fn batch_push_skips_unreadable_local_entry_instead_of_aborting() {
        let f = Fixture::new("batch-skip-junk-entry");
        f.write("a", "aaa", b"aaa-cipher");
        f.write("a", "zzz", b"zzz-cipher");
        let profiles_dir = f.root.join("a/profiles");
        std::fs::create_dir(profiles_dir.join("mmm.env")).unwrap();

        // Direct unit check: local_profile_names itself skips the directory
        // entry rather than erroring out.
        let names = local_profile_names(&profiles_dir).unwrap();
        assert_eq!(names, vec!["aaa".to_string(), "zzz".to_string()]);

        let reports = f.sync("a").push(&[], false).unwrap();
        let by_name: BTreeMap<&str, &Outcome> = outcomes(&reports).into_iter().collect();
        assert_eq!(by_name.len(), 2);
        assert_eq!(by_name["aaa"], &Outcome::Uploaded);
        assert_eq!(by_name["zzz"], &Outcome::Uploaded);
        assert!(!by_name.contains_key("mmm"));

        assert_eq!(f.remote.get("aaa").unwrap(), b"aaa-cipher");
        assert_eq!(f.remote.get("zzz").unwrap(), b"zzz-cipher");

        let state = SyncState::load(&f.root.join("a/sync-state.toml")).unwrap();
        assert_eq!(
            state.get("test", "aaa"),
            Some(sha256_hex(b"aaa-cipher").as_str())
        );
        assert_eq!(
            state.get("test", "zzz"),
            Some(sha256_hex(b"zzz-cipher").as_str())
        );
    }
}
