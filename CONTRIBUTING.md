# Contributing to gap

Normative process for developing, extending, reviewing, and releasing gap.
Prefer this document over tribal knowledge.

Agents: also read [`AGENTS.md`](AGENTS.md) for architecture rules. Process and
checklists live **here**.

---

## Principles

| Rule | Meaning |
| --- | --- |
| Small PRs | One problem per PR; reviewable diffs. |
| No invented APIs | Read existing types and docs first. |
| No compatibility debt | Remove obsolete paths; do not add dual APIs or migrations “for now.” |
| Secrets hygiene | Node keys (`GAP_SECRET`) are iroh `SecretKey`. Never log them at `info`. Never `#[derive(Debug)]` on wrappers that dump the key. |

License: contributions are dual-licensed [MIT](LICENSE-MIT) OR
[Apache-2.0](LICENSE-APACHE) as stated in `README.md`.

---

## Prerequisites

| Tool | Role |
| --- | --- |
| Rust stable ≥ 1.91 | Build, test. Workspace MSRV is 1.91 (iroh 1.0). |
| Rust nightly | `rustfmt` import grouping; Clippy workspace lints. |
| [`just`](https://github.com/casey/just) | Canonical task runner (`Justfile`). `Makefile` mirrors the same suite. |
| [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) | Licenses, bans, advisories, sources. |

```bash
rustup toolchain install stable nightly --component rustfmt,clippy
cargo install just cargo-deny
```

---

## Local quality gate

```bash
git clone https://github.com/qntx/gap.git
cd gap
just all
```

`just all` runs: **fmt → clippy-fix → deny → test**.

| Recipe | Purpose |
| --- | --- |
| `just all` | Default pre-PR gate |
| `just test` | `cargo test --workspace --all-features` |
| `just deny` | `cargo deny check` |
| `just fmt` / `just clippy` | Format / lint |

---

## Repository layout

```text
Cargo.toml                 workspace package versions, shared lints
deny.toml                  licenses / bans / advisories / sources
Justfile / Makefile        local gates (keep in sync with each other)
.github/workflows/
  ci.yml                   lint, test, deny
crates/
  gap/                     unpublished std CLI binary
  README.md                crate table
CONTRIBUTING.md            this file
CHANGELOG.md               Keep a Changelog
```

---

## Pull requests

1. Branch from `main`.
2. `just all` green.
3. Changelog under `[Unreleased]`.
4. One problem per PR.

CI is `qntx/workflows` `ci-rust.yml@v2` on `push`/`pull_request` to `main`.
Job timeout is 60 minutes (iroh compile).
