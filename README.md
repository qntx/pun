# gap

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![CI][ci-badge]][ci-url]
[![License][license-badge]][license-url]
[![Rust][rust-badge]][rust-url]

[crates-badge]: https://img.shields.io/crates/v/gap.svg
[crates-url]: https://crates.io/crates/gap
[docs-badge]: https://img.shields.io/docsrs/gap.svg
[docs-url]: https://docs.rs/gap
[ci-badge]: https://github.com/qntx/gap/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/qntx/gap/actions/workflows/ci.yml
[license-badge]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg
[license-url]: LICENSE-MIT
[rust-badge]: https://img.shields.io/badge/rust-edition%202024-orange.svg
[rust-url]: https://doc.rust-lang.org/edition-guide/

**P2P pipe in Rust — punch NAT, send a path, receive by ticket. blake3-verified streaming.**

`gap send` stands up an ephemeral iroh endpoint, imports a file or directory into a blob store, and prints a ticket. `gap receive` fetches that payload into the current directory. While the sender is running, the ticket is the capability: anyone who has it can fetch the blobs.

## Install

**macOS / Linux**

```sh
curl -fsSL https://sh.qntx.org/gap | sh
```

**Windows** (PowerShell)

```powershell
irm https://sh.qntx.org/gap/ps | iex
```

Or with Cargo — `cargo install gap`.

## Usage

```sh
gap send ./photo.jpg
gap receive <ticket>
```

`recv` is an alias for `receive`. Clipboard copies `gap receive {ticket}`.

```sh
gap send . --no-progress          # three-line stdout, no TTY chatter
gap send ./dir -c                 # copy the receive command (OSC 52)
gap send ./dir --relay disabled   # loopback / airgap
```

| | |
| --- | --- |
| `GAP_SECRET` | Optional hex node key. Random if unset. `--show-secret` prints it to stderr. |
| `RUST_LOG` | Tracing. `-v` / `-vv` are stats, not log levels. |
| `--relay` | `default` (n0), `disabled`, or a URL. |
| `--no-progress` | Hide bars. Non-TTY send prints exactly three ASCII lines. |
| `-c` / `--clipboard` | Copy `gap receive {ticket}` (TTY: press `c` after the ticket). |

Temp dirs: `./.gap-send-*` and `./.gap-recv-*`. Removed on graceful exit. Kill -9 leaves them; delete by hand.

Export names are `/`-split. Empty, `.`, `..`, NUL, `\`, and `/` in a component are rejected. Existing targets abort.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project shall be dual-licensed as above, without any additional terms or conditions.

---

<div align="center">

A **[QuantX](https://qntx.org)** open-source project.

<a href="https://qntx.org"><img alt="QuantX" width="369" src="https://raw.githubusercontent.com/qntx/.github/main/profile/qntx.svg" /></a>

Code is law. We write both.

</div>
