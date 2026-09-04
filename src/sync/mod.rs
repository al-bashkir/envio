//! Push and pull encrypted profiles to a remote store.
//!
//! Profiles are already encrypted on disk, so sync moves raw bytes. Conflict
//! detection compares SHA-256 hashes of the ciphertext on each side against
//! the hash recorded the last time the two sides agreed.

use sha2::{Digest, Sha256};

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
