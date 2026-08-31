# gap

P2P pipe in Rust. Punch through NAT and send files with blake3-verified streaming.

Unpublished command-line binary (`publish = false`). Dual-licensed MIT OR Apache-2.0.

Adapted from [n0-computer/sendme](https://github.com/n0-computer/sendme)
commit `8dda1e5383209e9027dd54430c77059ef51adc2e`.
Copyright N0, INC. Licensed Apache-2.0 OR MIT.
This crate's code was modified: module split, branding (`gap`, `GAP_SECRET`,
`.gap-*`), clippy rewrite, clipboard signaling without `unsafe`, and stricter
export-path validation.

## Usage

```text
gap send <file or directory>
gap receive <ticket>
```

`recv` is an alias for `receive`.

This creates an ephemeral iroh endpoint, imports the path into a blake3 blob
store, and prints a ticket. The receiver uses that ticket to fetch the data
into the current directory.

A ticket is capability-equivalent for the payload while the sender is running:
anyone who has the ticket can fetch the blobs.

### Environment

| Variable | Effect |
| --- | --- |
| `GAP_SECRET` | Optional hex iroh `SecretKey`. Generated randomly if unset. |
| `RUST_LOG` | `tracing-subscriber` filter. `-v` / `-vv` are not tracing. |

`--show-secret` prints the hex key to stderr. Do not log that value.

`-v` prints a generated `GAP_SECRET` (when the key was random) and extra receive
stats. `-vv` also lists collection entries. Tracing stays `RUST_LOG` only.

### Relays and progress

`--relay default|disabled|<URL>` (default: n0 relays via `presets::N0`).
`--no-progress` hides progress bars.

With `--no-progress`, default verbosity, and stdin not a TTY, `gap send` prints
exactly three ASCII lines on stdout:

```text
imported file <path>, <bytes>, hash <hex>
to get this data, use
gap receive <ticket>
```

### Temp directories

Send uses `./.gap-send-<32 hex chars>/`. Receive uses `./.gap-recv-<blake3 hex>/`.
These are deleted on graceful shutdown. Kill -9 / abort panics skip `Drop`;
delete leftover `.gap-*` directories by hand.

### Export path validation

Collection names are `/`-split. Each component is rejected if it is empty, `.`,
`..`, or contains NUL, `\`, or `/`. This is stricter than sendme, which only
rejected `/` and could follow `../` out of the current directory.

Export aborts if the target already exists.

### Clipboard

Default feature `clipboard`. `-c` / `--clipboard` copies `gap receive {ticket}`
via OSC 52. Press `c` after the ticket is printed (TTY only).

## Development

```bash
just all
```

Workspace MSRV is rustc 1.91 (iroh 1.0). See [CONTRIBUTING.md](CONTRIBUTING.md).
