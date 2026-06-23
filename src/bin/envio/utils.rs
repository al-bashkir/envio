use std::fs::File;
/// Utility functions used throughout the binary crate
use std::io::Write;
use std::path::PathBuf;

use envio::crypto::{create_encryption_type, gpg::get_gpg_keys, EncryptionType};
use envio::error::{Error, Result};
use envio::{Env, EnvVec};
use inquire::{min_length, Password, PasswordDisplayMode, Select};
use reqwest::Client;

const USE_PASSPHRASE_OPTION: &str = "Use passphrase (age) instead";

/// Shell flavor used for selecting the syntax of `export`/`unset` output.
#[cfg(target_family = "unix")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    /// Any other POSIX-ish shell. Treated as bash syntax for output.
    Other,
}

#[cfg(target_family = "unix")]
impl Shell {
    /// Map a shell binary path (the value of `$SHELL`) to a `Shell`.
    /// Match on the basename: `bash`, `zsh`, `fish`. Anything else is `Other`.
    pub fn from_shell_path(path: &str) -> Shell {
        let basename = path.rsplit('/').next().unwrap_or("");
        if basename.contains("bash") {
            Shell::Bash
        } else if basename.contains("zsh") {
            Shell::Zsh
        } else if basename.contains("fish") {
            Shell::Fish
        } else {
            Shell::Other
        }
    }
}

/// Detect the user's shell from the `$SHELL` environment variable.
/// Falls back to `Shell::Other` if `$SHELL` is unset or unrecognized.
#[cfg(target_family = "unix")]
pub fn detect_shell() -> Shell {
    match std::env::var("SHELL") {
        Ok(path) => Shell::from_shell_path(&path),
        Err(_) => Shell::Other,
    }
}

#[cfg(target_family = "unix")]
pub fn initalize_config() -> Result<()> {
    let configdir = get_configdir()?;
    if !configdir.exists() {
        std::fs::create_dir(&configdir)?;
        std::fs::create_dir(configdir.join("profiles"))?;
    }
    Ok(())
}
/// Get the config directory (`~/.envio`)
///
/// # Returns
/// - `PathBuf`: the config directory
pub fn get_configdir() -> Result<PathBuf> {
    Ok(envio::utils::get_configdir())
}

pub fn get_cwd() -> PathBuf {
    std::env::current_dir().unwrap()
}

/// Parse environment variables from a string
///
/// # Parameters
/// - `buffer`: &str - the buffer to parse
///
/// # Returns
/// - `Result<HashMap<String, String>>`: the parsed environment variables
pub fn parse_envs_from_string(buffer: &str) -> Result<EnvVec> {
    let mut envs_vec = EnvVec::new();

    for buf in buffer.lines() {
        if buf.is_empty() || !buf.contains('=') {
            continue;
        }

        let mut split = buf.split('=');

        let key = split.next();
        let mut value = split.next();

        if key.is_none() {
            return Err(Error::Msg("Can not parse key from buffer".to_string()));
        }

        if value.is_none() {
            value = Some("");
        }

        envs_vec.push(Env::from_key_value(
            key.unwrap().to_string(),
            value.unwrap().to_string(),
        ));
    }

    Ok(envs_vec)
}

/// Download a file from a url with a progress bar
///
/// # Parameters
/// - `url`: &str - the url to download the file from
/// - `file_name`: &str - the name of the file to save the downloaded file to
///
/// # Returns
/// - `Result<()>`: an empty result
pub async fn download_file(url: &str, file_name: &str) -> Result<()> {
    let bytes = Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| Error::Msg(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| Error::Msg(e.to_string()))?;

    File::create(file_name)?.write_all(&bytes)?;
    Ok(())
}

/// Interactive picker used by `envio create` when the user did not supply a
/// `-g`/`--gpg-key-fingerprint` flag.
///
/// Probes the system GPG keyring. If keys are found, shows a select prompt
/// containing one entry per key plus a sentinel entry that drops the user into
/// the age passphrase flow. An empty keyring falls back silently to the age
/// passphrase flow. A failed probe prints a warning to stderr (including the
/// underlying error detail) and then falls back to the age passphrase flow.
pub fn pick_encryption_for_new_profile(vim_mode: bool) -> Result<Box<dyn EncryptionType>> {
    #[cfg(target_family = "unix")]
    let keys_result: std::result::Result<Vec<(String, String)>, String> =
        get_gpg_keys().map_err(|e| e.to_string());

    #[cfg(target_family = "windows")]
    let keys_result: std::result::Result<Vec<(String, String)>, String> = match get_gpg_keys() {
        Some(keys) => Ok(keys),
        None => Err("keyring unavailable or gpg not installed".to_string()),
    };

    match keys_result {
        Ok(keys) if !keys.is_empty() => {
            let mut options: Vec<String> = keys.iter().map(|(label, _)| label.clone()).collect();
            options.push(USE_PASSPHRASE_OPTION.to_string());

            let ans = Select::new(
                "Select GPG key for encryption (or use passphrase):",
                options,
            )
            .with_vim_mode(vim_mode)
            .prompt()
            .map_err(|e| Error::Msg(e.to_string()))?;

            if ans == USE_PASSPHRASE_OPTION {
                age_passphrase_flow()
            } else {
                let fingerprint = keys
                    .into_iter()
                    .find_map(|(label, fp)| if label == ans { Some(fp) } else { None })
                    .ok_or_else(|| {
                        Error::Msg("Selected key not found in keyring list".to_string())
                    })?;
                create_encryption_type(fingerprint, "gpg")
            }
        }
        Ok(_) => age_passphrase_flow(),
        Err(detail) => {
            eprintln!(
                "Warning: GPG probe failed: {}. Falling back to passphrase.",
                detail
            );
            age_passphrase_flow()
        }
    }
}

fn age_passphrase_flow() -> Result<Box<dyn EncryptionType>> {
    let user_key = Password::new("Enter your encryption key:")
        .with_display_toggle_enabled()
        .with_display_mode(PasswordDisplayMode::Masked)
        .with_validator(min_length!(8))
        .with_formatter(&|_| String::from("Input received"))
        .with_help_message("Remember this key, you will need it to decrypt your profile later")
        .with_custom_confirmation_error_message("The keys don7't match.")
        .prompt()
        .map_err(|e| Error::Msg(e.to_string()))?;

    create_encryption_type(user_key, "age")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_from_path_bash() {
        assert_eq!(Shell::from_shell_path("/bin/bash"), Shell::Bash);
        assert_eq!(Shell::from_shell_path("/usr/local/bin/bash"), Shell::Bash);
    }

    #[test]
    fn shell_from_path_zsh() {
        assert_eq!(Shell::from_shell_path("/bin/zsh"), Shell::Zsh);
        assert_eq!(Shell::from_shell_path("/usr/bin/zsh-5.9"), Shell::Zsh);
    }

    #[test]
    fn shell_from_path_fish() {
        assert_eq!(Shell::from_shell_path("/usr/bin/fish"), Shell::Fish);
    }

    #[test]
    fn shell_from_path_other() {
        assert_eq!(Shell::from_shell_path("/bin/sh"), Shell::Other);
        assert_eq!(Shell::from_shell_path("/usr/bin/dash"), Shell::Other);
        assert_eq!(Shell::from_shell_path(""), Shell::Other);
    }
}
