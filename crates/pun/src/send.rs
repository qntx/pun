use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use indicatif::{HumanBytes, MultiProgress};
use iroh::protocol::Router;
use iroh::{RelayMode, SecretKey};
use iroh_blobs::api::TempTag;
use iroh_blobs::format::collection::Collection;
use iroh_blobs::provider::events::{
    ConnectMode, EventMask, EventSender, ProviderMessage, RequestMode,
};
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::{BlobFormat, BlobsProtocol};
use tokio::sync::{Notify, mpsc};

use crate::cli::SendArgs;
use crate::endpoint::bind_send_endpoint;
use crate::error::AppError;
use crate::import::import_paths;
use crate::progress::{set_draw_target, show_provide_progress, throughput_bps};
use crate::secret::get_or_create_secret;
use crate::store::TempStore;
use crate::ticket::{apply_options, print_hash};

struct Provider {
    router: Router,
    temp_tag: TempTag,
    size: u64,
    collection: Collection,
    elapsed: Duration,
}

pub(crate) async fn run(args: SendArgs) -> Result<(), AppError> {
    let secret = get_or_create_secret(args.common.verbose > 0)?;
    if args.common.show_secret {
        eprintln!("using secret key {}", hex::encode(secret.to_bytes()));
    }
    let cwd = std::env::current_dir().context("current directory")?;
    let mut temp = TempStore::create_send(&cwd, &args.path).await?;
    let mp = MultiProgress::new();
    let (progress_tx, progress_rx) = mpsc::channel(32);
    let progress = tokio::task::spawn(show_provide_progress(mp.clone(), progress_rx));
    let provider = match run_setup_interruptible(&temp, &args, secret, progress_tx, &mp).await {
        Ok(provider) => provider,
        Err(err) => {
            progress.abort();
            drop(temp.close().await);
            return Err(err);
        }
    };
    #[cfg(feature = "clipboard")]
    let ticket = announce_ticket(&provider, &args);
    #[cfg(not(feature = "clipboard"))]
    announce_ticket(&provider, &args);
    let interrupt = Arc::new(Notify::new());
    #[cfg(feature = "clipboard")]
    let clipboard_task =
        crate::clipboard::maybe_spawn(args.clipboard, ticket, Arc::clone(&interrupt));
    serve_until_interrupt(&interrupt).await;
    #[cfg(feature = "clipboard")]
    if let Some(handle) = clipboard_task {
        handle.abort();
    }
    shutdown_send(provider, &mut temp, progress).await
}

async fn run_setup_interruptible(
    temp: &TempStore,
    args: &SendArgs,
    secret: SecretKey,
    progress_tx: mpsc::Sender<ProviderMessage>,
    mp: &MultiProgress,
) -> Result<Provider, AppError> {
    tokio::select! {
        result = setup_provider(temp, args, secret, progress_tx, mp) => result,
        _ = tokio::signal::ctrl_c() => Err(AppError::Interrupted),
    }
}

async fn setup_provider(
    temp: &TempStore,
    args: &SendArgs,
    secret: SecretKey,
    progress_tx: mpsc::Sender<ProviderMessage>,
    mp: &MultiProgress,
) -> Result<Provider, AppError> {
    let t0 = Instant::now();
    let relay_mode = RelayMode::from(args.common.relay.clone());
    let endpoint = bind_send_endpoint(secret, &args.common, args.ticket_type).await?;
    set_draw_target(mp, args.common.no_progress);
    let blobs = blobs_protocol(temp.blobs()?, progress_tx);
    let (temp_tag, size, collection) =
        import_paths(args.path.clone(), blobs.store(), mp, args.common.jobs).await?;
    let router = Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();
    wait_endpoint_online(router.endpoint(), &relay_mode).await?;
    Ok(Provider {
        router,
        temp_tag,
        size,
        collection,
        elapsed: t0.elapsed(),
    })
}

fn blobs_protocol(store: &FsStore, progress_tx: mpsc::Sender<ProviderMessage>) -> BlobsProtocol {
    BlobsProtocol::new(
        store,
        Some(EventSender::new(
            progress_tx,
            EventMask {
                connected: ConnectMode::Notify,
                get: RequestMode::NotifyLog,
                ..EventMask::DEFAULT
            },
        )),
    )
}

async fn wait_endpoint_online(ep: &iroh::Endpoint, relay_mode: &RelayMode) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        if !matches!(relay_mode, RelayMode::Disabled) {
            ep.online().await;
        }
    })
    .await
    .context("timed out waiting for endpoint to come online")?;
    Ok(())
}

fn announce_ticket(provider: &Provider, args: &SendArgs) -> BlobTicket {
    let root_hash = provider.temp_tag.hash();
    let mut addr = provider.router.endpoint().addr();
    apply_options(&mut addr, args.ticket_type);
    let ticket = BlobTicket::new(addr, root_hash, BlobFormat::HashSeq);
    let entry_type = if args.path.is_file() {
        "file"
    } else {
        "directory"
    };
    println!(
        "imported {entry_type} {}, {}, hash {}",
        args.path.display(),
        HumanBytes(provider.size),
        print_hash(&root_hash, args.common.format),
    );
    if args.common.verbose > 1 {
        for (name, entry_hash) in provider.collection.iter() {
            println!("    {} {name}", print_hash(entry_hash, args.common.format));
        }
        println!(
            "{}s, {}/s",
            provider.elapsed.as_secs_f64(),
            HumanBytes(throughput_bps(provider.size, provider.elapsed))
        );
    }
    println!("to get this data, use");
    println!("pun receive {ticket}");
    ticket
}

async fn serve_until_interrupt(interrupt: &Notify) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = interrupt.notified() => {}
    }
}

async fn shutdown_send(
    provider: Provider,
    temp: &mut TempStore,
    progress: tokio::task::JoinHandle<anyhow::Result<()>>,
) -> Result<(), AppError> {
    drop(provider.temp_tag);
    println!("shutting down");
    tokio::time::timeout(Duration::from_secs(2), provider.router.shutdown())
        .await
        .context("router shutdown timed out")?
        .map_err(anyhow::Error::from)
        .context("router shutdown")?;
    temp.close().await?;
    drop(provider.router);
    drop(progress.await);
    Ok(())
}
