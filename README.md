<div align="center">
  <img src="assets/icon-color.svg" width="200px">
  <h1>envio</h1>
</div>

<div align="center">
  <h2 align="center">A Modern And Secure CLI Tool For Managing Environment Variables</h2>

  [![CICD](https://github.com/al-bashkir/envio/actions/workflows/CICD.yml/badge.svg)](https://github.com/al-bashkir/envio/actions/workflows/CICD.yml)
</div>

<div align='center'>
  <img alt="Demo" src="assets/envio-passphrase-final.gif" width="600">
</div>

<div align='center'>To see the GPG encryption demo go <a href="https://github.com/al-bashkir/envio/blob/main/assets/envio-gpg-final.gif">here</a></div>

## About

`envio` is an open source CLI tool that helps make managing environment variables a breeze. With `envio`, users can create encrypted profiles that contain a collection of environment variables associated with a specific project or use case. `envio` ensures security and simplifies the development process by allowing users to easily switch between profiles as needed and load them in their current terminal session for immediate use.

Some key features of `envio` include:

- **Encrypted** profiles through `passphrase` or `GPG` encryption
- **Load** profiles into your `terminal sessions`
- **Persistent** environment variables that are available in `future sessions`
- **Run** programs with your profiles
- **Importing** profiles stored on the internet into your local installation
- **Exporting** profiles to a file

Sound interesting? Check out the [repository](https://github.com/al-bashkir/envio) for more information on how to **install** and **use** the tool.

`envio` currently supports **Linux**, **MacOS** and **Windows**

## Install

Homebrew (macOS arm64 and Linux):

```bash
brew install al-bashkir/tools/envio
```

Linux users also need the system GPG libraries installed via their distro package manager:

- Debian/Ubuntu: `apt install libgpgme11 libgpg-error0`
- Fedora/RHEL: `dnf install gpgme libgpg-error`
- Arch: `pacman -S gpgme libgpg-error`

For other install methods (cargo, prebuilt binaries, Windows), see the [main repository](https://github.com/al-bashkir/envio).

## Loading a profile

`envio load` and `envio unload` print shell directives to stdout. Source them with `eval` (bash/zsh) or pipe to `source` (fish):

```sh
# bash / zsh
eval "$(envio load my-profile)"

# fish
envio load my-profile | source

# unload (same wrapper, in either shell)
eval "$(envio unload my-profile)"
```

## Syncing profiles between machines

`envio sync` copies your encrypted profiles to a remote and back. Profiles
stay encrypted in transit and at rest on the remote; the remote only ever
sees ciphertext, and no passphrase or GPG key is needed to sync.

```sh
envio sync remote add work      # interactive: pick s3, google_drive, or dir
envio sync push                 # upload every local profile
envio sync pull                 # on another machine: download every profile
envio sync status               # compare local and remote
envio sync push my-app --force  # overwrite the remote copy after a conflict
```

Push and pull refuse to overwrite a side that changed since the last sync.
`--force` overrides. With several remotes, pass `--remote NAME` or set
`default = "NAME"` at the top of `~/.envio/sync.toml`.

### S3 (AWS, MinIO, Cloudflare R2, Backblaze B2)

Credentials come from `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` (and
optional `AWS_SESSION_TOKEN`), or from `~/.aws/credentials` using
`AWS_PROFILE` or `[default]`. They are never written to `~/.envio/sync.toml`.
`remote add` asks for bucket, prefix, region and an optional endpoint for
non-AWS providers.

### Google Drive

Google requires an OAuth client that you own:

1. In the [Google Cloud Console](https://console.cloud.google.com/) create a
   project and enable the **Google Drive API**.
2. Under **APIs & Services → Credentials** create an **OAuth client ID** of
   type **TVs and Limited Input devices**.
3. Run `envio sync remote add drive`, choose `google_drive`, and paste the
   client ID and secret. envio prints a URL and a code; approve it in any
   browser. envio only gets access to files it created (scope `drive.file`).

Unlike S3 credentials, the OAuth client secret and the refresh token are
stored in `~/.envio/sync.toml` in **plaintext** (file mode `0600` on Unix,
so only your user account can read it — but they are not encrypted). Anyone
who reads that file can sync to your Drive folder until you revoke access
in your [Google Account permissions](https://myaccount.google.com/permissions).

### SMB, NFS, Proton Drive, or any mounted folder

Mount the share with your OS or the provider's desktop client, then add a
`dir` remote pointing at a folder inside it. Proton Drive has no public
API, so this is the supported way to use it.

## Contributors

<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/Vojtch159"><img src="https://avatars.githubusercontent.com/u/73985038?v=4?s=100" width="100px;" alt="Vojtch"/><br /><sub><b>Vojtch</b></sub></a><br /><a href="https://github.com/al-bashkir/envio/commits?author=Vojtch159" title="Documentation">📖</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/erjanmx"><img src="https://avatars.githubusercontent.com/u/4899432?v=4?s=100" width="100px;" alt="Erjan Kalybek"/><br /><sub><b>Erjan Kalybek</b></sub></a><br /><a href="https://github.com/al-bashkir/envio/commits?author=erjanmx" title="Documentation">📖</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/afh"><img src="https://avatars.githubusercontent.com/u/16507?v=4?s=100" width="100px;" alt="Alexis Hildebrandt"/><br /><sub><b>Alexis Hildebrandt</b></sub></a><br /><a href="https://github.com/al-bashkir/envio/commits?author=afh" title="Code">💻</a></td>
    </tr>
  </tbody>
</table>

## Contributing

Contributions to `envio` are always welcome! Please see the [Contributing Guidelines](CONTRIBUTING.md) for more information.

## License

This project is licensed under the [MIT](LICENSE-MIT) and the [Apache](LICENSE-APACHE) License
