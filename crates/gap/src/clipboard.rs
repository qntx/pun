use std::io::{IsTerminal, stdout};
use std::sync::Arc;

use crossterm::clipboard::CopyToClipboard;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use iroh_blobs::ticket::BlobTicket;
use n0_future::StreamExt;
use tokio::sync::Notify;

struct RawModeGuard {
    enabled: bool,
}

impl RawModeGuard {
    fn new() -> Self {
        match enable_raw_mode() {
            Ok(()) => Self { enabled: true },
            Err(err) => {
                eprintln!("Failed to enable raw mode: {err}");
                Self { enabled: false }
            }
        }
    }

    fn disable(&mut self) {
        if !self.enabled {
            return;
        }
        disable_raw_mode().unwrap_or_else(|err| eprintln!("Failed to disable raw mode: {err}"));
        self.enabled = false;
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        self.disable();
    }
}

fn add_to_clipboard(ticket: &BlobTicket) {
    execute!(
        stdout(),
        CopyToClipboard::to_clipboard_from(format!("gap receive {ticket}"))
    )
    .unwrap_or_else(|err| eprintln!("Failed to copy to clipboard: {err}"));
}

pub(crate) fn maybe_spawn(
    set_clipboard: bool,
    ticket: BlobTicket,
    interrupt: Arc<Notify>,
) -> Option<tokio::task::JoinHandle<()>> {
    if set_clipboard {
        add_to_clipboard(&ticket);
    }
    if !std::io::stdin().is_terminal() {
        return None;
    }
    println!("press c to copy command to clipboard, or use the --clipboard argument");
    Some(tokio::task::spawn(listen_keys(ticket, interrupt)))
}

async fn listen_keys(ticket: BlobTicket, interrupt: Arc<Notify>) {
    let mut raw = RawModeGuard::new();
    let mut events = EventStream::new();
    while let Some(item) = events.next().await {
        match item {
            Err(err) => eprintln!("Failed to process event: {err}"),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            })) => add_to_clipboard(&ticket),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            })) => {
                raw.disable();
                interrupt.notify_waiters();
            }
            _ => {}
        }
    }
}
