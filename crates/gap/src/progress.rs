#![allow(
    clippy::literal_string_with_formatting_args,
    reason = "indicatif ProgressStyle templates are not format! arguments"
)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use iroh_blobs::api::remote::GetProgressItem;
use iroh_blobs::get::{GetError, Stats};
use iroh_blobs::provider::events::{ProviderMessage, RequestUpdate};
use n0_future::{FuturesUnordered, StreamExt};
use tokio::sync::mpsc;
use tracing::{error, trace};

const TICK_MS: u64 = 250;

pub(crate) fn usize_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "display-only bytes/s"
)]
pub(crate) fn throughput_bps(bytes: u64, elapsed: Duration) -> u64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0;
    }
    (bytes as f64 / secs).floor() as u64
}

pub(crate) fn set_draw_target(mp: &MultiProgress, no_progress: bool) {
    let target = if no_progress {
        ProgressDrawTarget::hidden()
    } else {
        ProgressDrawTarget::stderr()
    };
    mp.set_draw_target(target);
}

fn bar_style(template: &'static str) -> ProgressStyle {
    ProgressStyle::with_template(template)
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
}

const fn tick(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

pub(crate) fn make_import_overall_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.enable_steady_tick(tick(TICK_MS));
    pb.set_style(bar_style(
        "{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len}",
    ));
    pb
}

pub(crate) fn make_import_item_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.enable_steady_tick(tick(TICK_MS));
    pb.set_style(bar_style(
        "{msg}{spinner:.green} XXXX [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
    ));
    pb
}

fn make_connect_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.set_style(bar_style(
        "{prefix}{spinner:.green} Connecting ... [{elapsed_precise}]",
    ));
    pb.set_prefix(format!("{} ", style("[1/4]").bold().dim()));
    pb.enable_steady_tick(tick(TICK_MS));
    pb
}

fn make_get_sizes_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.set_style(bar_style(
        "{prefix}{spinner:.green} Getting sizes... [{elapsed_precise}]",
    ));
    pb.set_prefix(format!("{} ", style("[2/4]").bold().dim()));
    pb.enable_steady_tick(tick(TICK_MS));
    pb
}

fn make_download_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.enable_steady_tick(tick(TICK_MS));
    pb.set_style(bar_style(
        "{prefix}{spinner:.green}{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} {binary_bytes_per_sec}",
    ));
    pb.set_prefix(format!("{} ", style("[3/4]").bold().dim()));
    pb.set_message("Downloading ...".to_owned());
    pb
}

pub(crate) fn make_export_overall_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.enable_steady_tick(tick(TICK_MS));
    pb.set_style(bar_style(
        "{prefix}{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {human_pos}/{human_len} {per_sec}",
    ));
    pb.set_prefix(format!("{}", style("[4/4]").bold().dim()));
    pb
}

pub(crate) fn make_export_item_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.enable_steady_tick(tick(100));
    pb.set_style(bar_style(
        "{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
    ));
    pb
}

pub(crate) fn take_connect_progress(mp: &MultiProgress) -> ProgressBar {
    mp.add(make_connect_progress())
}

pub(crate) fn take_get_sizes_progress(mp: &MultiProgress) -> ProgressBar {
    mp.add(make_get_sizes_progress())
}

#[derive(Debug)]
struct PerConnectionProgress {
    endpoint_id: String,
    requests: BTreeMap<u64, ProgressBar>,
}

fn lock_conns(
    m: &Mutex<BTreeMap<u64, PerConnectionProgress>>,
) -> MutexGuard<'_, BTreeMap<u64, PerConnectionProgress>> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

async fn per_request_progress(
    mp: MultiProgress,
    connection_id: u64,
    request_id: u64,
    connections: Arc<Mutex<BTreeMap<u64, PerConnectionProgress>>>,
    mut rx: irpc::channel::mpsc::Receiver<RequestUpdate>,
) {
    let pb = mp.add(ProgressBar::hidden());
    let endpoint_id = if let Some(connection) = lock_conns(&connections).get_mut(&connection_id) {
        connection.requests.insert(request_id, pb.clone());
        connection.endpoint_id.clone()
    } else {
        error!("got request for unknown connection {connection_id}");
        return;
    };
    pb.set_style(bar_style(
        "{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
    ));
    while let Ok(Some(msg)) = rx.recv().await {
        match msg {
            RequestUpdate::Started(msg) => {
                pb.set_message(format!(
                    "n {} r {}/{} i {} # {}",
                    endpoint_id,
                    connection_id,
                    request_id,
                    msg.index,
                    msg.hash.fmt_short()
                ));
                pb.set_length(msg.size);
            }
            RequestUpdate::Progress(msg) => {
                pb.set_position(msg.end_offset);
            }
            RequestUpdate::Completed(_) | RequestUpdate::Aborted(_) => {
                if let Some(entry) = lock_conns(&connections).get_mut(&connection_id) {
                    entry.requests.remove(&request_id);
                }
            }
        }
    }
    pb.finish_and_clear();
    mp.remove(&pb);
}

fn on_provider_message(
    item: ProviderMessage,
    mp: &MultiProgress,
    connections: &Arc<Mutex<BTreeMap<u64, PerConnectionProgress>>>,
    tasks: &mut FuturesUnordered<n0_future::boxed::BoxFuture<()>>,
) {
    trace!("got event {item:?}");
    match item {
        ProviderMessage::ClientConnectedNotify(msg) => {
            let endpoint_id = msg
                .endpoint_id
                .map_or_else(|| "?".to_owned(), |id| id.fmt_short().to_string());
            lock_conns(connections).insert(
                msg.connection_id,
                PerConnectionProgress {
                    requests: BTreeMap::new(),
                    endpoint_id,
                },
            );
        }
        ProviderMessage::ConnectionClosed(msg) => {
            let closed = lock_conns(connections).remove(&msg.connection_id);
            if let Some(connection) = closed {
                for pb in connection.requests.values() {
                    pb.finish_and_clear();
                    mp.remove(pb);
                }
            }
        }
        ProviderMessage::GetRequestReceivedNotify(msg) => {
            let request_id = msg.request_id;
            let connection_id = msg.connection_id;
            let connections = Arc::clone(connections);
            let mp = mp.clone();
            tasks.push(Box::pin(per_request_progress(
                mp,
                connection_id,
                request_id,
                connections,
                msg.rx,
            )));
        }
        _ => {}
    }
}

pub(crate) async fn show_provide_progress(
    mp: MultiProgress,
    mut recv: mpsc::Receiver<ProviderMessage>,
) -> anyhow::Result<()> {
    let connections = Arc::new(Mutex::new(BTreeMap::new()));
    let mut tasks = FuturesUnordered::new();
    loop {
        tokio::select! {
            biased;
            item = recv.recv() => {
                let Some(item) = item else {
                    break;
                };
                on_provider_message(item, &mp, &connections, &mut tasks);
            }
            Some(()) = tasks.next(), if !tasks.is_empty() => {}
        }
    }
    while tasks.next().await.is_some() {}
    Ok(())
}

pub(crate) async fn show_download_progress(
    mp: MultiProgress,
    mut recv: mpsc::Receiver<u64>,
    local_size: u64,
    total_size: u64,
) -> anyhow::Result<()> {
    let op = mp.add(make_download_progress());
    op.set_length(total_size);
    while let Some(offset) = recv.recv().await {
        op.set_position(local_size + offset);
    }
    op.finish_and_clear();
    Ok(())
}

pub(crate) async fn on_get_progress_item(
    item: GetProgressItem,
    tx: &mpsc::Sender<u64>,
) -> anyhow::Result<Option<Stats>> {
    match item {
        GetProgressItem::Progress(offset) => {
            tx.send(offset).await.ok();
            Ok(None)
        }
        GetProgressItem::Done(value) => Ok(Some(value)),
        GetProgressItem::Error(cause) => anyhow::bail!(show_get_error(cause)),
    }
}

pub(crate) fn show_get_error(e: GetError) -> GetError {
    match &e {
        GetError::InitialNext { source, .. } => eprintln!(
            "{}",
            style(format!("initial connection error: {source}")).yellow()
        ),
        GetError::ConnectedNext { source, .. } => {
            eprintln!("{}", style(format!("connected error: {source}")).yellow());
        }
        GetError::AtBlobHeaderNext { source, .. } => eprintln!(
            "{}",
            style(format!("reading blob header error: {source}")).yellow()
        ),
        GetError::Decode { source, .. } => {
            eprintln!("{}", style(format!("decoding error: {source}")).yellow());
        }
        GetError::IrpcSend { source, .. } => eprintln!(
            "{}",
            style(format!("error sending over irpc: {source}")).yellow()
        ),
        GetError::AtClosingNext { source, .. } => {
            eprintln!("{}", style(format!("error at closing: {source}")).yellow());
        }
        GetError::BadRequest { .. } => eprintln!("{}", style("bad request").yellow()),
        GetError::LocalFailure { source, .. } => {
            eprintln!("{} {source:?}", style("local failure").yellow());
        }
    }
    e
}
