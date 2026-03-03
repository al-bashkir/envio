use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::Command;

use bincode::Options;
use chrono::{Duration, Utc};
use colored::Colorize;
use dirs::cache_dir;
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder;

const CACHE_BINC_LIMIT_BYTES: u64 = 4 * 1024;

fn cache_bincode_opts_fixint() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .with_limit(CACHE_BINC_LIMIT_BYTES)
}

fn cache_bincode_opts_varint() -> impl Options {
    bincode::DefaultOptions::new()
        .with_varint_encoding()
        .allow_trailing_bytes()
        .with_limit(CACHE_BINC_LIMIT_BYTES)
}

fn read_cache_data(cache_file: &PathBuf) -> Option<CacheData> {
    let file = File::open(cache_file).ok()?;
    let reader = BufReader::new(file);

    if let Ok(data) = cache_bincode_opts_fixint().deserialize_from(reader) {
        return Some(data);
    }

    let file = File::open(cache_file).ok()?;
    let reader = BufReader::new(file);

    if let Ok(data) = cache_bincode_opts_varint().deserialize_from(reader) {
        // Best-effort migration to the current cache format.
        let _ = write_cache_data(cache_file, &data);
        return Some(data);
    }

    None
}

fn write_cache_data(cache_file: &PathBuf, cache_data: &CacheData) -> Result<(), bincode::Error> {
    let file = File::create(cache_file)?;
    let mut writer = BufWriter::new(file);
    cache_bincode_opts_fixint().serialize_into(&mut writer, cache_data)
}

#[derive(Serialize, Deserialize)]
struct CacheData {
    version: String,
    last_update_time: chrono::DateTime<Utc>,
}

fn get_cache_dir() -> Option<PathBuf> {
    let app_name = env!("CARGO_PKG_NAME");
    if let Some(cache_dir) = cache_dir() {
        let app_cache_dir = cache_dir.join(app_name);
        if !app_cache_dir.exists() {
            if let Err(e) = create_dir_all(&app_cache_dir) {
                println!(
                    "{}: Failed to create cache directory {}: {}",
                    "Error".red(),
                    app_cache_dir.display(),
                    e
                );
                return None;
            }
        }
        Some(app_cache_dir)
    } else {
        println!("{}: Failed to get cache directory", "Error".red());
        None
    }
}

/// Get the latest version from the cache file, GitHub API or git If the cache
/// file doesn't exist, fetch the latest version from the Github API or git and
/// create the cache file If the cache file is older than 7 days, fetch the
/// latest version from GitHub API If the GitHub API fails, fetch the latest
/// version from git
///
/// # Returns
/// - `Version`: the latest version
pub fn get_latest_version() -> Version {
    let cache_dir = if let Some(cache_dir) = get_cache_dir() {
        cache_dir
    } else {
        println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
        return Version::parse("0.0.0").unwrap();
    };

    let cache_file = cache_dir.join("cache.bin");

    let cache_data: CacheData = if let Some(data) = read_cache_data(&cache_file) {
        data
    } else {
        // Cache file either doesn't exist or is not readable.
        // If it's corrupted or from an older encoding, remove it and recreate.
        let _ = std::fs::remove_file(&cache_file);

        let cache_data = CacheData {
            version: fetch_latest_version("0.0.0").to_string(),
            last_update_time: Utc::now(),
        };

        if let Err(e) = write_cache_data(&cache_file, &cache_data) {
            println!("{}: Failed to write cache file: {}", "Error".red(), e);
        }

        cache_data
    };

    let seven_days_ago = Utc::now() - Duration::days(7);

    if cache_data.last_update_time <= seven_days_ago {
        let latest_version = fetch_latest_version(&cache_data.version);

        let mut new_cache_data = cache_data;
        new_cache_data.last_update_time = Utc::now();
        new_cache_data.version = latest_version.to_string();

        if let Err(e) = write_cache_data(&cache_file, &new_cache_data) {
            println!("{}: Failed to write cache file: {}", "Error".red(), e);
            println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
            return Version::parse("0.0.0").unwrap();
        }

        latest_version
    } else if let Ok(version) = Version::parse(&cache_data.version) {
        version
    } else {
        println!("{}: Failed to parse version from cache file", "Error".red());
        println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
        Version::parse("0.0.0").unwrap()
    }
}

/// Fetch the latest version from GitHub API or git If the GitHub API fails,
/// fetch the latest version from git If the git command fails, return the
/// fallback version
///
/// # Parameters
/// - `fallback_version`: &str - The fallback version
///
/// # Returns
/// - `Version`: The latest version
fn fetch_latest_version(fallback_version: &str) -> Version {
    run_fetch_version_from_github_api().unwrap_or_else(|| {
        if let Some(val) = fetch_version_from_git() {
            val
        } else {
            println!("{}:  Failed to get latest version", "Error".red());
            println!(
                "{}: You can still use envio but won't be notified about new versions!",
                "Warning".yellow()
            );
            if let Ok(version) = Version::parse(fallback_version) {
                version
            } else {
                println!("{}: Failed to parse fallback version", "Error".red());
                println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
                Version::parse("0.0.0").unwrap()
            }
        }
    })
}

fn run_fetch_version_from_github_api() -> Option<Version> {
    let rt = if let Ok(val) = Builder::new_current_thread().enable_all().build() {
        val
    } else {
        return None;
    };

    rt.block_on(fetch_version_from_github_api())
}

async fn fetch_version_from_github_api() -> Option<Version> {
    let url = "https://api.github.com/repos/al-bashkir/envio/releases/latest";
    let client = reqwest::Client::new();
    let res = if let Ok(val) = client.get(url).header("User-Agent", "envio").send().await {
        val
    } else {
        return None;
    };

    match res.status() {
        reqwest::StatusCode::OK => {
            let body = if let Ok(val) = res.text().await {
                val
            } else {
                return None;
            };

            if body.contains("tag_name") {
                let mut tag_name = body.split("tag_name").collect::<Vec<&str>>()[1]
                    .split('\"')
                    .collect::<Vec<&str>>()[2];

                tag_name = tag_name.trim_start_matches('v');
                let latest_version = if let Ok(val) = Version::parse(tag_name) {
                    val
                } else {
                    return None;
                };

                return Some(latest_version);
            }

            None
        }

        _ => None,
    }
}

fn fetch_version_from_git() -> Option<Version> {
    if Command::new("git").arg("--version").output().is_err() {
        println!("{}: Git is not installed", "Error".red());
        return None;
    }

    let owner = "al-bashkir";
    let repo = "envio";
    let output = Command::new("git")
        .arg("ls-remote")
        .arg(format!("https://github.com/{}/{}.git", owner, repo))
        .output()
        .unwrap();
    let reader = BufReader::new(output.stdout.as_slice());
    let mut latest_tag = None;

    for line in reader.lines().map_while(|x| x.ok()) {
        let parts: Vec<_> = line.split('\t').collect();
        if parts.len() != 2 {
            continue;
        }
        let (ref_name, _) = (parts[1], parts[0]);
        if ref_name.starts_with("refs/tags/") {
            let tag = ref_name.trim_start_matches("refs/tags/").to_owned();
            latest_tag =
                latest_tag.map_or(Some(tag.clone()), |latest| Some(std::cmp::max(latest, tag)));
        }
    }

    if let Some(mut tag) = latest_tag {
        tag = tag.trim_start_matches('v').to_string();
        if let Ok(version) = Version::parse(&tag) {
            return Some(version);
        }
    }

    None
}
