---
description: Prepare an envio release — bump version, update changelog, commit, and open a release branch
user_invocable: true
argument: Optional explicit bump level (`major`/`minor`/`patch`) or full version (e.g. `0.7.0`). If omitted, auto-detect from conventional commits.
---

You are preparing a release of the `envio` crate. Follow these steps precisely.

## 0. Context

- `envio` is a single Rust crate. Version lives in `Cargo.toml` (`[package].version`) and is mirrored into `Cargo.lock`.
- Release pipeline is fully automated by two workflows:
  - `.github/workflows/release.yml` — runs on push to `main` when `Cargo.toml` changes. Reads `[package].version`, creates and pushes `v<version>` tag if missing.
  - `.github/workflows/CICD.yml` — runs on tag push `v*`. Builds cross-platform artifacts and publishes the GitHub release.
- This skill's job is to land the version bump on `main`. The workflows do the tagging and release. **Do not** tag locally or run any `gh release` / `gh workflow` commands.
- Conventional Commits are used throughout the history. The `CHANGELOG.md` keeps an in-progress `# Unreleased` section at the top.

## 1. Determine current state

- Read current version: `grep '^version' Cargo.toml` (top-level `[package]` entry only — ignore the second `version` field that may appear under dependencies).
- Latest released tag: `git tag --sort=-creatordate | grep '^v' | head -1`.
- Unreleased commits: `git log <latest-tag>..origin/main --oneline`.
- If no commits since the latest tag, inform the user nothing to release and stop.
- Also check `CHANGELOG.md` for an existing `# Unreleased` section — entries there must be preserved into the new release section.

## 2. Determine the version bump

If the user passed `$ARGUMENTS`:

- `major` / `minor` / `patch` → use that bump level
- `X.Y.Z` literal → use that as the new version
- otherwise → treat as override and confirm with the user

Otherwise, derive from conventional commits since the latest tag:

- **major** — any commit subject contains `BREAKING CHANGE`, or uses a `!` after the type (`feat!:`, `fix!:`, etc.), **or** the `CHANGELOG.md` `# Unreleased` section has a `## BREAKING CHANGES` block
- **minor** — any commit starts with `feat:` or `feat(<scope>):`
- **patch** — everything else (`fix:`, `chore:`, `ci:`, `docs:`, `refactor:`, …)

Pre-1.0 caveat: `envio` is still `0.x`. Even with a `BREAKING CHANGE`, the bump should usually be **minor** (e.g. `0.6.x → 0.7.0`) rather than `1.0.0`. Default to minor for breaking changes in 0.x and ask the user to confirm before promoting to a full major.

Parse current version from `Cargo.toml`, apply the bump, and present:

- current → new version
- the bump level and why
- the commit list grouped by type

Wait for user confirmation. They may override the bump or version.

## 3. Update `Cargo.toml`

Bump the top-level `[package].version` to the new version. Do not touch any other version fields.

## 4. Refresh `Cargo.lock`

Run a quick build/check so `Cargo.lock` updates the `envio` entry to the new version:

```sh
cargo check --locked || cargo check
```

If `--locked` fails because the lockfile is stale, drop `--locked` so the lockfile regenerates. Include the resulting `Cargo.lock` change in the release commit.

## 5. Update `CHANGELOG.md`

- Rename the existing `# Unreleased` heading to `# v<new-version>` (no date — match the format of past releases in this file).
- Merge in any commits that arrived since `# Unreleased` was last touched. Group commit subjects under the matching subsection:
  - `feat:` → `## Features`
  - `fix:` → `## Bug Fixes`
  - `chore:`, `refactor:`, `perf:`, `docs:`, `style:`, `build:`, `ci:` → `## Others`
  - Anything with `BREAKING CHANGE` or `!` → `## BREAKING CHANGES` (and write the migration notes — do not just paste the subject)
- Strip the conventional-commit prefix from each line. Keep PR numbers / `#NN` references.
- Add a fresh empty `# Unreleased` section at the top for future work.

If the file already had hand-written content under `# Unreleased` (breaking notes, migration guide), preserve it verbatim under the new `# v<new-version>` heading — do not rewrite the user's prose.

## 6. Generated artifacts

`build.rs` regenerates `completions/` and `man/` on every build. The `cargo check` from step 4 may produce diffs in those directories. Per `AGENTS.md`:

> Do not commit generated diffs unless you intentionally changed the CLI surface.

Inspect with `git status` and `git diff completions/ man/`:

- If diffs match real CLI changes already landed since the last release, include them.
- If diffs are spurious (timestamps, ordering), discard with `git checkout -- completions/ man/`.

## 7. Run the test suite

Match the checks CI runs on MSRV (1.75.0) so the tag build doesn't blow up:

```sh
cargo fmt -- --check
cargo clippy --locked --all-targets
cargo test --locked
```

If any step fails, stop and surface the failure to the user. Do not "fix" lint/test failures as part of the release commit — fixes belong in their own commits before the release branch.

## 8. Commit on a release branch

- Create a branch from `origin/main`: `release/v<new-version>`
- Stage `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` (and only intentional `completions/`/`man/` diffs)
- Commit message: `chore: prepare v<new-version> release`
- **Do not** add a `Co-Authored-By:` trailer (per global user preference)

## 9. Push branch and hand off

- Push branch: `git push -u origin release/v<new-version>`
- Print a summary for the user:
  - branch name
  - new version
  - changelog excerpt for the new section
  - list of staged files
  - command to open the PR: `gh pr create --base main --head release/v<new-version> --title "chore: release v<new-version>"`

Stop here. Per project convention, **do not** open the PR automatically — the user runs `gh pr create` (or the GitHub UI) themselves.

## 10. Release pipeline (automatic — nothing to do)

After the release PR merges to `main`:

1. `.github/workflows/release.yml` fires on the `Cargo.toml` change, reads the new version, and pushes `v<new-version>` tag.
2. `.github/workflows/CICD.yml` fires on the tag push, builds the cross-platform artifacts, and publishes the GitHub release.

Do not tag locally. Do not run `gh release create` or `gh workflow run`. The workflows own this step.
