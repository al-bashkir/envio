//! `envio sync ...` subcommands: interactive remote setup and push/pull/status
//! output. All logic lives in `envio::sync`; this file only maps paths and
//! prints.

use std::path::PathBuf;

use colored::Colorize;
use inquire::{Password, PasswordDisplayMode, Select, Text};

use envio::error::{Error, Result};
use envio::sync::{check_name, gdrive, Outcome, Remote, Report, Status, Sync, SyncConfig};

use crate::clap_app::{RemoteAction, SyncAction};

struct Paths {
    config: PathBuf,
    state: PathBuf,
    profiles: PathBuf,
}

fn paths() -> Paths {
    let dir = envio::utils::get_configdir();
    Paths {
        config: dir.join("sync.toml"),
        state: dir.join("sync-state.toml"),
        profiles: dir.join("profiles"),
    }
}

fn prompt_err(e: inquire::InquireError) -> Error {
    Error::Msg(e.to_string())
}

fn text(prompt: &str, default: Option<&str>) -> Result<String> {
    let mut t = Text::new(prompt);
    if let Some(d) = default {
        t = t.with_default(d);
    }
    Ok(t.prompt().map_err(prompt_err)?.trim().to_string())
}

fn prompt_remote(vim_mode: bool) -> Result<Remote> {
    let kind = Select::new("Backend:", vec!["s3", "google_drive", "dir"])
        .with_vim_mode(vim_mode)
        .prompt()
        .map_err(prompt_err)?;
    match kind {
        "s3" => {
            let bucket = text("Bucket:", None)?;
            let prefix = text("Prefix:", Some("envio"))?;
            let region = text("Region:", Some("us-east-1"))?;
            let endpoint = text(
                "Endpoint (leave blank for AWS; e.g. http://localhost:9000 for MinIO):",
                None,
            )?;
            Ok(Remote::S3 {
                bucket,
                prefix,
                region,
                endpoint: if endpoint.is_empty() {
                    None
                } else {
                    Some(endpoint)
                },
            })
        }
        "google_drive" => {
            let client_id = text("OAuth client ID:", None)?;
            let client_secret = Password::new("OAuth client secret:")
                .with_display_mode(PasswordDisplayMode::Masked)
                .without_confirmation()
                .prompt()
                .map_err(prompt_err)?;
            let refresh_token = gdrive::device_login(&client_id, &client_secret)?;
            let token = gdrive::access_token(&client_id, &client_secret, &refresh_token)?;
            let folder_id = gdrive::create_folder(&token, "envio")?;
            println!("Created folder `envio` in My Drive");
            Ok(Remote::GoogleDrive {
                client_id,
                client_secret,
                refresh_token,
                folder_id,
            })
        }
        _ => {
            let path = PathBuf::from(text("Directory path:", None)?);
            if !path.is_dir() {
                return Err(Error::Sync(format!(
                    "`{}` is not a directory",
                    path.display()
                )));
            }
            Ok(Remote::Dir { path })
        }
    }
}

fn describe(remote: &Remote) -> String {
    match remote {
        Remote::S3 {
            bucket,
            prefix,
            region,
            endpoint,
        } => format!(
            "s3  {}/{}  ({}{})",
            bucket,
            prefix,
            region,
            endpoint
                .as_deref()
                .map(|e| format!(", {}", e))
                .unwrap_or_default()
        ),
        Remote::GoogleDrive { folder_id, .. } => format!("google_drive  folder {}", folder_id),
        Remote::Dir { path } => format!("dir  {}", path.display()),
    }
}

fn status_label(status: Status) -> colored::ColoredString {
    match status {
        Status::UpToDate => "up to date".green(),
        Status::LocalAhead => "local ahead".yellow(),
        Status::RemoteAhead => "remote ahead".yellow(),
        Status::Conflict => "conflict".red(),
        Status::OnlyLocal => "only local".cyan(),
        Status::OnlyRemote => "only remote".cyan(),
    }
}

/// Print one line per profile. Returns an error if anything was refused,
/// missing or failed, so the process exits non-zero.
fn print_reports(reports: &[Report]) -> Result<()> {
    let mut problems = 0;
    for r in reports {
        let msg = match &r.outcome {
            Outcome::Uploaded => "uploaded".green(),
            Outcome::Downloaded => "downloaded".green(),
            Outcome::UpToDate => "up to date".green(),
            Outcome::Refused(status) => {
                problems += 1;
                format!("{} (use --force)", status_label(*status)).red()
            }
            Outcome::Missing(why) => {
                problems += 1;
                why.red()
            }
            Outcome::Failed(e) => {
                problems += 1;
                format!("failed: {}", e).red()
            }
        };
        println!("{:<24} {}", r.profile, msg);
    }
    if problems > 0 {
        return Err(Error::Sync(format!(
            "{} of {} profiles not synced",
            problems,
            reports.len()
        )));
    }
    Ok(())
}

pub fn run(action: &SyncAction, vim_mode: bool) -> Result<()> {
    let p = paths();
    match action {
        SyncAction::Remote { action } => {
            let mut cfg = SyncConfig::load(&p.config)?;
            match action {
                RemoteAction::Add { name } => {
                    check_name(name)?;
                    if cfg.remotes.contains_key(name) {
                        return Err(Error::Sync(format!("remote `{}` already exists", name)));
                    }
                    let remote = prompt_remote(vim_mode)?;
                    cfg.remotes.insert(name.clone(), remote);
                    cfg.save(&p.config)?;
                    println!("{} remote {}", "Added".green(), name);
                    if cfg.remotes.len() > 1 && cfg.default.is_none() {
                        println!(
                            "Several remotes configured: pass --remote or set `default = \"{}\"` in {}",
                            name,
                            p.config.display()
                        );
                    }
                }
                RemoteAction::List => {
                    if cfg.remotes.is_empty() {
                        println!("No remotes. Run `envio sync remote add <NAME>`.");
                    }
                    for (name, remote) in &cfg.remotes {
                        let marker = if cfg.default.as_deref() == Some(name) {
                            " (default)"
                        } else {
                            ""
                        };
                        println!("{}{}  {}", name.green(), marker, describe(remote));
                    }
                }
                RemoteAction::Remove { name } => {
                    if cfg.remotes.remove(name).is_none() {
                        return Err(Error::Sync(format!("remote `{}` not found", name)));
                    }
                    if cfg.default.as_deref() == Some(name) {
                        cfg.default = None;
                    }
                    cfg.save(&p.config)?;
                    println!("{} remote {}", "Removed".green(), name);
                }
            }
        }
        SyncAction::Push {
            profiles,
            remote,
            force,
        } => {
            let cfg = SyncConfig::load(&p.config)?;
            let (name, remote) = cfg.select(remote.as_deref())?;
            let sync = Sync {
                remote_name: name,
                remote,
                profiles_dir: &p.profiles,
                state_path: &p.state,
            };
            print_reports(&sync.push(profiles, *force)?)?;
        }
        SyncAction::Pull {
            profiles,
            remote,
            force,
        } => {
            let cfg = SyncConfig::load(&p.config)?;
            let (name, remote) = cfg.select(remote.as_deref())?;
            let sync = Sync {
                remote_name: name,
                remote,
                profiles_dir: &p.profiles,
                state_path: &p.state,
            };
            print_reports(&sync.pull(profiles, *force)?)?;
        }
        SyncAction::Status { remote } => {
            let cfg = SyncConfig::load(&p.config)?;
            let (name, remote) = cfg.select(remote.as_deref())?;
            let sync = Sync {
                remote_name: name,
                remote,
                profiles_dir: &p.profiles,
                state_path: &p.state,
            };
            for (profile, status) in sync.status()? {
                println!("{:<24} {}", profile, status_label(status));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(profile: &str, outcome: Outcome) -> Report {
        Report {
            profile: profile.to_string(),
            outcome,
        }
    }

    #[test]
    fn a_reported_failure_makes_the_process_exit_non_zero() {
        // A profile sync cannot name must not disappear into a "success"
        // run: the engine reports it as `Failed`, and that has to become a
        // non-zero exit here.
        let reports = vec![
            report("work", Outcome::Uploaded),
            report(
                ".hidden",
                Outcome::Failed("profile name not supported by sync".into()),
            ),
        ];
        let err = print_reports(&reports).unwrap_err().to_string();
        assert!(err.contains("1 of 2"), "{}", err);
    }

    #[test]
    fn a_clean_run_is_ok() {
        let reports = vec![
            report("work", Outcome::Uploaded),
            report("staging", Outcome::UpToDate),
            report("new", Outcome::Downloaded),
        ];
        assert!(print_reports(&reports).is_ok());
    }
}
