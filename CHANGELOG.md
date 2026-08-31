# Changelog

All notable changes to this workspace are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- GitHub Release workflow (`release-rust.yml@v2.0.0`, `bin`/`package` = `gap`).
- Dependabot version updates (cargo + github-actions).
- `.gitattributes`, markdownlint config.

### Changed

- Crate is crates.io-publishable (`publish = false` removed).
- README license footer matches QuantX layout. sendme attribution retained.

## [0.1.0] - 2026-08-31

### Added

- `gap` CLI: `gap send` / `gap receive`, adapted from n0-computer/sendme `8dda1e53`.
- Workspace scaffold: dual MIT/Apache-2.0 license, nightly fmt/clippy gates, cargo-deny, CI v2.
