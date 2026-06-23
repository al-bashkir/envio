use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use colored::Colorize;
use dirs::cache_dir;
use semver::Version;
use tokio::runtime::Builder;

struct CacheData {
    version: String,
    last_update_time: DateTime<Utc>,
}

/// Cache file format: two lines, `<version>\n<rfc3339-timestamp>`.
fn read_cache_data(cache_file: &PathBuf) -> Option<CacheData> {
    let mut s = String::new();
    File::open(cache_file).ok()?.read_to_string(&mut s).ok()?;
    let (version, ts) = s.trim().split_once('\n')?;
    Some(CacheData {
        version: version.to_string(),
        last_update_time: ts.parse().ok()?,
    })
}

fn write_cache_data(cache_file: &PathBuf, cache_data: &CacheData) -> std::io::Result<()> {
    let mut file = File::create(cache_file)?;
    write!(
        file,
        "{}\n{}",
        cache_data.version,
        cache_data.last_update_time.to_rfc3339()
    )
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

/// Compare the latest released version against this build and print a notice.
/// Hits the network (GitHub API); only called via `version --check`.
pub fn check_for_update() {
    let latest = get_latest_version();
    let current = Version::parse(env!("BUILD_VERSION")).unwrap_or_else(|_| {
        println!("{}: Failed to parse current version", "Error".red());
        Version::new(0, 0, 0)
    });

    if latest > current {
        println!(
            "{}: {} -> {}",
            "New version available".yellow(),
            current,
            latest
        );
    } else {
        println!("{}", "You are on the latest version".green());
    }
}

/// Get the latest version from the cache file or the GitHub API. If the cache
/// file doesn't exist or is older than 7 days, fetch from the GitHub API and
/// rewrite the cache.
///
/// # Returns
/// - `Version`: the latest version
pub fn get_latest_version() -> Version {
    let cache_dir = if let Some(cache_dir) = get_cache_dir() {
        cache_dir
    } else {
        println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
        return Version::new(0, 0, 0);
    };

    let cache_file = cache_dir.join("cache.txt");

    let cache_data: CacheData = if let Some(data) = read_cache_data(&cache_file) {
        data
    } else {
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

        let new_cache_data = CacheData {
            version: latest_version.to_string(),
            last_update_time: Utc::now(),
        };

        if let Err(e) = write_cache_data(&cache_file, &new_cache_data) {
            println!("{}: Failed to write cache file: {}", "Error".red(), e);
        }

        latest_version
    } else if let Ok(version) = Version::parse(&cache_data.version) {
        version
    } else {
        println!("{}: Failed to parse version from cache file", "Error".red());
        println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
        Version::new(0, 0, 0)
    }
}

/// Fetch the latest version from the GitHub API, falling back to the given
/// version string if the request fails.
fn fetch_latest_version(fallback_version: &str) -> Version {
    run_fetch_version_from_github_api().unwrap_or_else(|| {
        println!("{}:  Failed to get latest version", "Error".red());
        println!(
            "{}: You can still use envio but won't be notified about new versions!",
            "Warning".yellow()
        );
        Version::parse(fallback_version).unwrap_or_else(|_| {
            println!("{}: Failed to parse fallback version", "Error".red());
            println!("{}: Using 0.0.0 as fallback version", "Warning".yellow());
            Version::new(0, 0, 0)
        })
    })
}

fn run_fetch_version_from_github_api() -> Option<Version> {
    let rt = Builder::new_current_thread().enable_all().build().ok()?;
    rt.block_on(fetch_version_from_github_api())
}

async fn fetch_version_from_github_api() -> Option<Version> {
    let url = "https://api.github.com/repos/al-bashkir/envio/releases/latest";
    let client = reqwest::Client::new();
    let res = client
        .get(url)
        .header("User-Agent", "envio")
        .send()
        .await
        .ok()?;

    if res.status() != reqwest::StatusCode::OK {
        return None;
    }

    let body = res.text().await.ok()?;
    if !body.contains("tag_name") {
        return None;
    }

    let tag_name = body.split("tag_name").collect::<Vec<&str>>()[1]
        .split('\"')
        .collect::<Vec<&str>>()[2]
        .trim_start_matches('v');

    Version::parse(tag_name).ok()
}
