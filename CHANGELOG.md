# Changelog

All notable changes to this workspace are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- GitHub Release workflow (`release-rust.yml@v2.0.0`, `bin`/`package` = `gap`).
- Dependabot version updates (cargo + github-actions).
- `.gitattributes`, markdownlint config.

### Changed

- Crate is crates.io-publishable (`publish = false` removed).
- README: kobe-style badges, `sh.qntx.org/gap` install, compact usage. sendme attribution moved to `NOTICE`.
- Direct dependencies upgraded to current crates.io (iroh `1.1.0`; clap `4.6.6`; tokio `1.53.1`; iroh-blobs remains `0.103.0`, latest).
- CI caller no longer sets job-level `timeout-minutes` (invalid on `uses:`); pass `timeout-minutes: 60` as a reusable-workflow input.

### Removed

- `CONTRIBUTING.md`, `Makefile`.

## [0.1.0] - 2026-08-31

### Added

- `gap` CLI: `gap send` / `gap receive`, adapted from n0-computer/sendme `8dda1e53`.
- Workspace scaffold: dual MIT/Apache-2.0 license, nightly fmt/clippy gates, cargo-deny, CI v2.
