use clap::CommandFactory;
use clap_complete::{generate_to, shells::*};
use std::{env, fs, path::PathBuf, process};

include!("src/bin/envio/clap_app.rs");

fn main() {
    let mut cmd = ClapApp::command();
    let app_name = cmd.get_name().to_string();

    let completions_dir = "completions";

    if let Err(e) = create_dir(completions_dir) {
        panic!("Error: {}", e);
    }

    if let Err(e) = generate_completions(&mut cmd, &app_name, completions_dir) {
        panic!("Error: {}", e);
    }

    if let Err(e) = inject_profile_completers(completions_dir) {
        panic!("Error injecting profile completers: {}", e);
    }

    let manpage_dir = "man";
    if let Err(e) = create_dir(manpage_dir) {
        panic!("Error: {}", e);
    }

    if let Err(e) = generate_manpages(cmd, manpage_dir) {
        panic!("Error: {}", e);
    }

    let build_timestamp: String = get_buildtimestamp();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_timestamp);
    println!("cargo:rustc-env=BUILD_VERSION={}", get_version());
}

/// Generate manpages for the CLI application
fn generate_manpages(cmd: clap::Command, out_dir: &str) -> std::io::Result<()> {
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer).unwrap();

    std::fs::write(PathBuf::from(out_dir).join("envio.1"), buffer)?;

    Ok(())
}

/// Generate completions for the CLI application
fn generate_completions(
    cmd: &mut clap::Command,
    app_name: &str,
    outdir: &str,
) -> std::io::Result<()> {
    generate_to(Bash, cmd, app_name, outdir)?;
    generate_to(Zsh, cmd, app_name, outdir)?;
    generate_to(Fish, cmd, app_name, outdir)?;
    generate_to(PowerShell, cmd, app_name, outdir)?;

    Ok(())
}

const COMPLETION_SENTINEL: &str = "# envio: dynamic profile completion END";

/// Append/patch each generated completion script with a block that makes the
/// profile-name positions complete to existing profile names by shelling out
/// to `envio list --profiles --no-pretty-print`.
fn inject_profile_completers(outdir: &str) -> std::io::Result<()> {
    use std::path::Path;
    let outdir = Path::new(outdir);

    inject_bash(&outdir.join("envio.bash"))?;
    inject_zsh(&outdir.join("_envio"))?;
    inject_fish(&outdir.join("envio.fish"))?;
    inject_powershell(&outdir.join("_envio.ps1"))?;

    Ok(())
}

const BASH_OVERRIDE: &str = r#"

# envio: dynamic profile completion BEGIN
# Appended by build.rs after clap_complete. Provides profile-name completion
# for: add, load, unload, launch, remove, update, export, and `list -n <value>`.

_envio_profiles() {
    envio list --profiles --no-pretty-print 2>/dev/null
}

_envio_with_profile_completion() {
    _envio "$@"

    local cur sub i
    cur="${COMP_WORDS[COMP_CWORD]}"

    sub=""
    for (( i=1; i<COMP_CWORD; i++ )); do
        case "${COMP_WORDS[i]}" in
            -*) ;;
            *) sub="${COMP_WORDS[i]}"; break ;;
        esac
    done

    case "$sub" in
        add|load|unload|launch|remove|update|export)
            local seen_positionals=0 j
            for (( j=i+1; j<COMP_CWORD; j++ )); do
                case "${COMP_WORDS[j]}" in
                    -*) ;;
                    *) seen_positionals=$((seen_positionals+1)) ;;
                esac
            done
            if [[ $seen_positionals -eq 0 && "$cur" != -* ]]; then
                local IFS=$'\n'
                COMPREPLY=( $(compgen -W "$(_envio_profiles)" -- "$cur") )
            fi
            ;;
        list)
            local prev="${COMP_WORDS[COMP_CWORD-1]}"
            if [[ "$prev" == "-n" || "$prev" == "--profile-name" ]]; then
                local IFS=$'\n'
                COMPREPLY=( $(compgen -W "$(_envio_profiles)" -- "$cur") )
            fi
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _envio_with_profile_completion -o nosort -o bashdefault -o default envio
else
    complete -F _envio_with_profile_completion -o bashdefault -o default envio
fi
# envio: dynamic profile completion END
"#;

fn inject_bash(path: &std::path::Path) -> std::io::Result<()> {
    let body = std::fs::read_to_string(path)?;
    if body.contains(COMPLETION_SENTINEL) {
        return Ok(());
    }
    std::fs::write(path, format!("{}{}", body, BASH_OVERRIDE))
}

const ZSH_HELPER: &str = r#"

# envio: dynamic profile completion BEGIN
_envio_profiles() {
    local -a profiles
    profiles=("${(@f)$(envio list --profiles --no-pretty-print 2>/dev/null)}")
    if (( ${#profiles[@]} )); then
        _describe -t profiles 'profile' profiles
    fi
}
# envio: dynamic profile completion END
"#;

const ZSH_PROFILE_SUBCOMMANDS: &[&str] = &[
    "add", "load", "unload", "launch", "remove", "update", "export",
];

fn inject_zsh(path: &std::path::Path) -> std::io::Result<()> {
    let body = std::fs::read_to_string(path)?;
    if body.contains(COMPLETION_SENTINEL) {
        return Ok(());
    }

    let mut out = String::with_capacity(body.len() + ZSH_HELPER.len());
    let mut current_stanza: Option<String> = None;

    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('(') {
            if let Some(close) = rest.find(')') {
                let name = &rest[..close];
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    && rest[close + 1..].trim().is_empty()
                {
                    current_stanza = Some(name.to_string());
                    out.push_str(line);
                    continue;
                }
            }
        }
        if trimmed.starts_with(";;") {
            current_stanza = None;
            out.push_str(line);
            continue;
        }

        let in_target_stanza = current_stanza
            .as_deref()
            .map(|s| ZSH_PROFILE_SUBCOMMANDS.contains(&s))
            .unwrap_or(false);
        let in_list_stanza = current_stanza.as_deref() == Some("list");

        if in_target_stanza && line.contains("':profile_name:'") {
            out.push_str(&line.replace("':profile_name:'", "':profile_name:_envio_profiles'"));
            continue;
        }
        if in_list_stanza {
            if line.contains("'-n+[]:PROFILE_NAME: '") {
                out.push_str(&line.replace(
                    "'-n+[]:PROFILE_NAME: '",
                    "'-n+[]:PROFILE_NAME:_envio_profiles '",
                ));
                continue;
            }
            if line.contains("'--profile-name=[]:PROFILE_NAME: '") {
                out.push_str(&line.replace(
                    "'--profile-name=[]:PROFILE_NAME: '",
                    "'--profile-name=[]:PROFILE_NAME:_envio_profiles '",
                ));
                continue;
            }
        }

        out.push_str(line);
    }

    out.push_str(ZSH_HELPER);
    std::fs::write(path, out)
}

const FISH_OVERRIDE: &str = r#"

# envio: dynamic profile completion BEGIN
function __envio_profiles
    envio list --profiles --no-pretty-print 2>/dev/null
end

complete -c envio -n '__fish_seen_subcommand_from add load unload launch remove update export' -f -a '(__envio_profiles)'
complete -c envio -n '__fish_seen_subcommand_from list' -s n -l profile-name -r -f -a '(__envio_profiles)'
# envio: dynamic profile completion END
"#;

fn inject_fish(path: &std::path::Path) -> std::io::Result<()> {
    let body = std::fs::read_to_string(path)?;
    if body.contains(COMPLETION_SENTINEL) {
        return Ok(());
    }
    std::fs::write(path, format!("{}{}", body, FISH_OVERRIDE))
}

const POWERSHELL_PROFILE_BRANCHES: &[&str] = &[
    "'envio;add'",
    "'envio;load'",
    "'envio;unload'",
    "'envio;launch'",
    "'envio;remove'",
    "'envio;update'",
    "'envio;export'",
];

const POWERSHELL_PROFILE_INJECT: &str = r#"            envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
"#;

const POWERSHELL_LIST_INJECT: &str = r#"            $prev = if ($commandElements.Count -ge 2) { $commandElements[$commandElements.Count - 2].Value } else { '' }
            if ($prev -eq '-n' -or $prev -eq '--profile-name') {
                envio list --profiles --no-pretty-print 2>$null | ForEach-Object {
                    [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
                }
            }
"#;

const POWERSHELL_TRAILER: &str = "\n# envio: dynamic profile completion END\n";

fn inject_powershell(path: &std::path::Path) -> std::io::Result<()> {
    let body = std::fs::read_to_string(path)?;
    if body.contains(COMPLETION_SENTINEL) {
        return Ok(());
    }

    let mut out = String::with_capacity(body.len() + 1024);
    let mut current_branch: Option<String> = None;
    let mut injected_in_current_branch = false;

    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start();

        if let Some(end) = trimmed.find('{') {
            let head = trimmed[..end].trim();
            if head.starts_with('\'') && head.ends_with('\'') {
                current_branch = Some(head.to_string());
                injected_in_current_branch = false;
                out.push_str(line);
                continue;
            }
        }

        if !injected_in_current_branch {
            if let Some(branch) = current_branch.as_deref() {
                if trimmed.starts_with("break") {
                    if POWERSHELL_PROFILE_BRANCHES.contains(&branch) {
                        out.push_str(POWERSHELL_PROFILE_INJECT);
                        injected_in_current_branch = true;
                    } else if branch == "'envio;list'" {
                        out.push_str(POWERSHELL_LIST_INJECT);
                        injected_in_current_branch = true;
                    }
                }
            }
        }

        if trimmed.starts_with('}') {
            current_branch = None;
            injected_in_current_branch = false;
        }

        out.push_str(line);
    }

    out.push_str(POWERSHELL_TRAILER);
    std::fs::write(path, out)
}

/// Get the version of the build
fn get_version() -> String {
    let mut cmd = process::Command::new("git");

    cmd.arg("describe");
    cmd.arg("--abbrev=0");
    cmd.arg("--tags=0");

    if let Ok(status) = cmd.status() {
        if status.success() {
            if let Ok(output) = cmd.output() {
                return format!("{:?}", output);
            }
        }
    }

    println!("Error: Cannot get build version using `git` using CARGO_PKG_VERSION");
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get the build timestamp
fn get_buildtimestamp() -> String {
    return chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
}

/// Auxilliary function to create a directory
fn create_dir(dir_name: &str) -> Result<(), std::io::Error> {
    fs::create_dir_all(dir_name)?;

    Ok(())
}
