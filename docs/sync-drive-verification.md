# Google Drive sync: manual verification checklist

`envio sync`'s Google Drive backend needs a Google Cloud project, a real
OAuth client, and a human to approve the device-code login in a browser.
None of that is available in CI or to an unattended agent, so this path has
**not** been exercised live before merge. Run this checklist by hand once,
on a machine with a browser, before relying on the Drive backend.

## 1. Create the OAuth client

1. Go to the [Google Cloud Console](https://console.cloud.google.com/) and
   create a new project (or pick an existing one you're happy to use for
   this).
2. Open **APIs & Services → Library**, search for **Google Drive API**, and
   click **Enable**.
3. Open **APIs & Services → OAuth consent screen**. Configure it (an
   "External" app in "Testing" mode is fine for personal use — you don't
   need to publish it). Add your own Google account as a test user if
   prompted.
4. Open **APIs & Services → Credentials → Create Credentials → OAuth client
   ID**.
5. For **Application type**, choose **TVs and Limited Input devices**. This
   is required — it's what makes the device-code flow (`envio` prints a URL
   and a code, you approve it in any browser) work. A "Desktop app" or "Web
   application" client type will not work with `envio`'s login flow.
6. Save the client. Copy the **Client ID** and **Client secret** — `envio`
   will ask for both.

## 2. Run the command sequence

Run these in order, in a normal shell with a browser available (not a
throwaway `$HOME`, unless you want a disposable test — see note below).

| # | Command | Expected output |
|---|---------|------------------|
| 1 | `envio sync remote add drive` | Prompts: `Backend:` → choose `google_drive`; `OAuth client ID:` → paste it; `OAuth client secret:` → paste it (masked). Then envio prints a URL (`https://www.google.com/device`) and a short code. Open the URL in any browser, enter the code, sign in, and approve. Back in the terminal, envio prints `Created folder \`envio\` in My Drive` and `Added remote drive`. |
| 2 | `envio sync push` | One line per local profile: `a  uploaded` (or whichever profile names you have locally). Exit code 0. |
| 3 | `envio sync push` (again, no changes) | `a  up to date`. Exit code 0 — nothing re-uploaded. |
| 4 | Edit the local profile, e.g. `printf "cipher-a2" > ~/.envio/profiles/a.env` | (no output — just a local file change) |
| 5 | `envio sync push` | `a  uploaded` — local is ahead, so it re-uploads. Exit code 0. |
| 6 | `rm ~/.envio/profiles/a.env` | (no output — removes the local copy) |
| 7 | `envio sync pull` | `a  downloaded`. Exit code 0. |
| 8 | `cat ~/.envio/profiles/a.env` | `cipher-a2` — the content from step 5, round-tripped through Drive. |

Note: you can run this against a scratch `$HOME` the same way the MinIO
check does (`HOME=$(mktemp -d) sh -c '...'`, creating `$HOME/.envio/profiles`
and seeding `a.env` first) if you'd rather not touch your real `~/.envio`.
The device-code approval step still needs a real browser and a real Google
account either way.

## 3. Check the Drive web UI

Open [Google Drive](https://drive.google.com/) in the browser for the
account you approved with.

- [ ] A folder named `envio` exists in **My Drive**.
- [ ] Inside it there is **exactly one** file named `a.env` — not two.
  This is the check that matters most: Google Drive happily allows two
  files with the same name in the same folder, so if `envio`'s "update
  existing file" logic were broken (e.g. it always created instead of
  ever patching), step 5 above would silently leave a duplicate `a.env`
  behind instead of overwriting the first one. Right-click the file →
  **Manage versions** (or check "Last modified") to confirm it was
  updated at step 5's timestamp, not created fresh.
- [ ] The file's content, if you download it, is raw ciphertext
  (`cipher-a2` in this walkthrough) — not something Drive can render as
  text/preview meaningfully. This is expected: `envio` never sends
  plaintext to Drive.
- [ ] No other files were created in the folder besides the profiles you
  pushed.

## 4. Clean up

- Revoke `envio`'s access from
  [Google Account → Security → Third-party apps with account access](https://myaccount.google.com/permissions)
  if this was a throwaway test.
- Delete the `envio` folder from Drive if you don't want to keep the test
  data.
- If you used a scratch `$HOME`, remove it (`rm -rf "$HOME"`).

## Why this matters

Roughly half of `src/sync/gdrive.rs` — the device-code exchange, the
folder-listing query, and above all the "does `a.env` already exist in the
folder → PATCH vs POST" branch in `put()` — has no offline test coverage,
because it depends on Google's live API responses. The offline unit tests
cover the pure logic (conflict detection, name validation, TOML
round-tripping) but cannot exercise the actual HTTP calls. This checklist
is the only way to confirm the wire path works until someone runs it by
hand.
