---
name: release
description: Cut and publish a gin-rummy release — version bump, CHANGELOG dating, Release commit, bare git tag, cargo publish, GitHub release. Use when asked to release, publish, or cut a new version of this crate.
---

# Releasing gin-rummy

The process is manual and follows the conventions shared by jdh8's crates
(`contract-bridge`, `pons`, `dds-bridge`).  Follow the steps in order; ask
before proceeding if any check fails.

## 1. Preflight

- Working tree clean, on `main`, up to date with `origin/main`.
- The full verification gate passes (see CLAUDE.md):

  ```sh
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
  cargo test --all-features
  cargo test
  ```

- If `Cargo.toml` dependencies changed since the last release, run the
  minimal-versions check (nightly), then restore `Cargo.lock`.
- CI on `main` is green: `gh run list --branch main --limit 3`.

## 2. Pick the version

Cargo semver for 0.x: breaking changes bump the minor (`0.1.z` → `0.2.0`);
additive features and fixes bump the patch.  Adding fields to the
`#[non_exhaustive]` `Rules` (and friends) is additive.  Adding a `Phase`
variant is breaking.

## 3. Bump and date

- Set `version` in `Cargo.toml`.
- Run `cargo check` so the committed `Cargo.lock` picks up the new version.
- In `CHANGELOG.md`, retitle the pending section from
  `## [X.Y.Z] — Unreleased` to `## [X.Y.Z] — YYYY-MM-DD` (em dash, today's
  date).  The section content should already exist; releases only date it.
- If the MSRV changed, it must already be bumped in BOTH `Cargo.toml`
  (`rust-version`) and the CI test matrix in
  `.github/workflows/rust.yml` — verify.  The MSRV stays out of the README
  on purpose.

## 4. Package sanity

- `cargo package --list` — review that only intended files ship and that
  `README.md` and the `description` in `Cargo.toml` say what the crates.io
  page should say (0.1.0 shipped a stale "game engine" wording; do not
  repeat that).
- `cargo publish --dry-run` for a final build of the exact artifact.

## 5. Commit, tag, push, publish

```sh
git add -A
git commit -m "Release X.Y.Z"     # exactly this title, no prefix
git tag X.Y.Z                     # bare version, NO "v" prefix
git push origin main X.Y.Z
cargo publish
```

## 6. GitHub release

Create a release on the tag whose notes are the CHANGELOG section for this
version (without the `## [X.Y.Z]` heading itself):

```sh
gh release create X.Y.Z --title "X.Y.Z" --notes "<changelog section body>"
```

## 7. Post-release

- Confirm the version on <https://crates.io/crates/gin-rummy>.
- docs.rs builds arrive with a delay; check
  <https://docs.rs/gin-rummy> later rather than treating a pending build as
  a failure.
- Report the published version, tag, and release URL to the user.
