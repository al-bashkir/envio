use std::fs::File;
/// Utility functions used throughout the binary crate
use std::io::Write;
use std::path::PathBuf;

use envio::error::{Error, Result};
use envio::{Env, EnvVec};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;

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
/// Get the home directory
///
/// # Returns
/// - `PathBuf`: the home directory
pub fn get_homedir() -> Result<PathBuf> {
    match dirs::home_dir() {
        Some(home) => Ok(home),
        None => Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        ))),
    }
}

/// Get the config directory
///
/// # Returns
/// - `PathBuf`: the config directory
pub fn get_configdir() -> Result<PathBuf> {
    Ok(get_homedir()?.join(".envio"))
}

pub fn contains_path_separator(s: &str) -> bool {
    s.contains('/') || s.contains('\\')
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
    let client = Client::new();
    let mut resp = if let Err(e) = client.get(url).send().await {
        return Err(Error::Msg(e.to_string()));
    } else {
        client.get(url).send().await.unwrap()
    };

    let mut file = File::create(file_name)?;

    let mut content_length = if resp.content_length().is_none() {
        return Err(Error::Msg("Content length is not available".to_string()));
    } else {
        resp.content_length().unwrap()
    };

    let pb = ProgressBar::new(content_length);

    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    while let Some(chunk) = resp.chunk().await.unwrap() {
        let chunk_size = chunk.len();
        file.write_all(&chunk)?;

        pb.inc(chunk_size as u64);
        content_length -= chunk_size as u64;
    }

    pb.finish();
    Ok(())
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
