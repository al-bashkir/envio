# Change Log

# Unreleased

## Features

* `envio sync` pushes and pulls encrypted profiles to a remote, so the same profiles are available on several machines:

  ```sh
  envio sync remote add work   # interactive: s3, google_drive, or dir
  envio sync push              # upload every local profile
  envio sync pull              # on another machine, download every profile
  envio sync status            # compare local and remote
  ```

  Three backends are supported: S3-compatible storage (AWS, MinIO, Cloudflare R2, Backblaze B2), Google Drive, and any mounted directory, which is how SMB, NFS and Proton Drive are used. Profiles move as ciphertext, so syncing never needs your passphrase or GPG key and the remote only ever holds encrypted bytes. `push` and `pull` take profile names to limit them to a subset, and `--remote` selects between several configured remotes.

  Push and pull refuse to overwrite a side that changed since the last sync, comparing the SHA-256 of the encrypted bytes. `--force` overrides and is destructive in both directions: `push --force` discards the other machine's version, `pull --force` overwrites your local copy, neither with a backup. Sync never deletes, so a profile you remove locally survives on the remote and comes back with the next `envio sync pull`.

  Remotes are configured in `~/.envio/sync.toml`. S3 credentials are never written there; they come from `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` or `~/.aws/credentials`. The Google Drive OAuth client secret and refresh token are stored in that file in plaintext, with mode `0600` on Unix. Google Drive needs an OAuth client you create yourself; see the README for the Google Cloud Console steps.

## Others

* `envio sync` only accepts profile names made of letters, digits, spaces, `-`, `_` and `.`, and a name must not start or end with `.` or a space. Every other command is unaffected: a profile named outside that set still works normally with `create`, `load`, `list`, `add`, `update` and `export`. Only syncing refuses it, and `envio sync push` reports it as `profile name not supported by sync` and exits non-zero rather than skipping it quietly.

  Names are restricted because a profile name becomes an object key on the remote and a file name on every machine you sync to. Non-ASCII names such as `café` are excluded specifically because macOS and Linux normalize Unicode filenames differently, so one visible name could end up as two separate remote copies.

  To sync a profile whose name is not supported, re-create it under a supported name:

  ```sh
  envio export old-name -f /tmp/profile.env
  envio create new-name -f /tmp/profile.env
  envio remove old-name
  rm /tmp/profile.env
  ```

  Renaming the file in `~/.envio/profiles/` directly does not work. A profile stores its own name and path inside the encrypted payload, so later edits would write back to the old file.

# v0.7.0

## Features

* `envio version --check` queries for a newer release on demand. The previous behavior of checking for updates on every command (and its progress UI) has been removed, so normal commands no longer make a network call at startup.

## Others

* Dropped unused dependencies: `indicatif`, `url`, and `color-eyre`.
* Removed dead code across `profile`, `crypto`, and `utils`; simplified the `load!` macro and `Profile` API.
* Added Homebrew install instructions to the docs.
* `cargo fmt` on `build.rs` (#15).

# v0.6.6

## Features

* Shell completions now suggest existing profile names for `envio add`, `load`, `unload`, `launch`, `remove`, `update`, `export`, and the `-n`/`--profile-name` value of `envio list`. `create` and `import` are intentionally excluded because their profile name must not already exist. Re-source the completion file (or reinstall it from `envio completion <shell>`) after upgrading.

# v0.6.5

## BREAKING CHANGES

* `envio load` now writes shell directives to stdout. Wrap with `eval "$(envio load <profile>)"` (bash/zsh) or `envio load <profile> | source` (fish). It no longer modifies any files or your shell rc.
* `envio unload` now requires a `<profile_name>` argument on Unix (Windows already required it). Output goes to stdout; wrap the same way as `load`.
* `~/.envio/setenv.sh` is no longer created or used. Existing files can be deleted: `rm ~/.envio/setenv.sh`.

## Migration for existing users

Older versions of `envio` appended a block like the following to your shell rc on first run:

```
# envio DO NOT MODIFY
source ~/.envio/setenv.sh
```

(For fish users: `bass source ~/.envio/setenv.sh`.)

With this version `~/.envio/setenv.sh` is no longer written, so that line points to a missing file and your shell will print a `No such file or directory` error on every new shell. Remove the `# envio DO NOT MODIFY` line and the `source ...` (or `bass source ...`) line that follows it from:

* `~/.bashrc`
* `~/.zshrc`
* `~/.config/fish/config.fish`

(Only the file matching your shell will have the block.) After removing the lines, start a new shell — the error should be gone.

## Features

* Auto-detect GPG keys and fall back to passphrase in `envio create` (#11)

# v0.6.1
## Features
* Users can now pass in the `-v` (or `--update-values`) argument to optionally update the values of their envs in the `update` command

## Bug Fixes
* Implement backward compatibility with older profile handling #67
* Fix issue where adding a comment or expiration date to a new or updated environment variable prompted input for all existing environment variables in the profile #70

# v0.6.0
## Features
* Launch command as positional argument by @Rubensei in https://github.com/envio-cli/envio/pull/41
* Stream output from launched command by @afh in https://github.com/envio-cli/envio/pull/40
* Add --envs option to export by @afh in https://github.com/envio-cli/envio/pull/42
* Allow selection with vi keys by @afh in https://github.com/envio-cli/envio/pull/46
* Add support for nix flakes by @afh in https://github.com/envio-cli/envio/pull/51

## Bug Fixes
* Fix non-truncated shellscript by @jerome-jutteau in https://github.com/envio-cli/envio/pull/49

## Others
* Bump clap_complete from 4.2.1 to 4.5.1 by @dependabot in https://github.com/envio-cli/envio/pull/37
* Bump inquire from 0.6.1 to 0.7.0 by @afh in https://github.com/envio-cli/envio/pull/47

## New Contributors
* @Rubensei made their first contribution in https://github.com/envio-cli/envio/pull/41
* @jerome-jutteau made their first contribution in https://github.com/envio-cli/envio/pull/49

# v0.5.1
## Improvements
* Improve `add`, `launch`, `update` and `remove` commands usage
* Allow environment variable value to have an equal sign #33 

## Bug Fixes
* Fix encryption identification issue #36 
* Use `CARGO_PKG_VERSION` when git is not installed by @afh in https://github.com/envio-cli/envio/pull/31
* Only list profiles ending with `.env` by @afh in https://github.com/envio-cli/envio/pull/32
* Prevent setenv.sh script from including entire stdout #27

## Other
* Bump chrono from 0.4.24 to 0.4.33 by @dependabot in https://github.com/envio-cli/envio/pull/30
* Bump actions/checkout from 3 to 4 by @dependabot in https://github.com/envio-cli/envio/pull/28
# v0.5.0
## Features
* Add GPG encryption for user profiles, See [Usage](./docs/usage.md)
* Add flags for using the CLI, See [Usage](./docs/usage.md)

## Bug Fixes
* Fix issue #17 where envio assumed shell config file was in home directory and would panic if it was not found; envio now prompts users to pass in their shell config if it cannot find it

* Fix bug where envio would exit without doing its first time setup routine if it could not find the shell config; envio now checks for the config directory to determine if it is a fresh install or not

## Other
* Switched to `age` from `magic_crypt` for passphrase encryption method #13

This document records all notable changes to [envio](https://github.com/humblepenguinn/envio).
# v0.4.1
## Bug Fixes
* Fix slow startup times due to slow update fetching

# v0.4.0
## Features
* Add new argument `--no-pretty-print` to `envio list` command, See [Usage](./docs/usage.md)
* Add support for `fish` shell #9 
* 
## Bug Fixes
* Fix Security Vulnerability, See [Here](./docs/envio-profile-loading-update.md)

## Other
* Fix readme typo by @erjanmx in https://github.com/humblepenguinn/envio/pull/10
* docs: add erjanmx as a contributor for doc by @allcontributors in https://github.com/humblepenguinn/envio/pull/11
* Bump tokio from 1.26.0 to 1.27.0 by @dependabot in https://github.com/humblepenguinn/envio/pull/12

# v0.3.0
# Features
* Users can now create new profiles using files and also pass in environment variables #8 
* Added support for the `fish` shell #9 

# Bug Fixes
* Both the config and profiles directory are created at startup, if they do not exist #8 

# Other
* docs: add Vojtch159 as a contributor for doc by @allcontributors in https://github.com/humblepenguinn/envio/pull/7
* Bump clap_complete from 4.1.5 to 4.2.0 by @dependabot in https://github.com/humblepenguinn/envio/pull/4
* Bump reqwest from 0.11.14 to 0.11.16 by @dependabot in https://github.com/humblepenguinn/envio/pull/5

# v0.2.0
## Features
* In addition to being able to load and unload profiles in current terminal sessions, users can now use the `envio launch` sub command to launch programs with specific profiles see [Usage](./docs/usage.md)
* envio now automatically looks for updates

## Bug Fixes
* Users do not need to type in their key twice when modifying environment variables in a profile

## Other
* Update [usage.md](./docs/usage.md) by @Vojtch159 in https://github.com/humblepenguinn/envio/pull/1
* Update [usage.md](./docs/usage.md) to include usage of `launch` sub command
* Update [CODEOWNERS](./CODEOWNERS)
* Update [README](./README.md) with new `gif` showing the new `launch` sub command

## New Contributors
* @Vojtch159 made their first contribution in https://github.com/humblepenguinn/envio/pull/1

# v0.1.0

- Inital Release


