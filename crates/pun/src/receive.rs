use anyhow::Context;
use indicatif::{HumanBytes, HumanDuration, MultiProgress};
use iroh::Endpoint;
use iroh_blobs::HashAndFormat;
use iroh_blobs::api::Store;
use iroh_blobs::api::remote::LocalInfo;
use iroh_blobs::format::collection::Collection;
use iroh_blobs::get::Stats;
use iroh_blobs::get::request::get_hash_seq_and_sizes;
use n0_future::StreamExt;
use tokio::sync::mpsc;
use tracing::trace;

use crate::cli::ReceiveArgs;
use crate::endpoint::bind_recv_endpoint;
use crate::error::AppError;
use crate::export::export_collection;
use crate::progress::{
    on_get_progress_item, set_draw_target, show_download_progress, show_get_error,
    take_connect_progress, take_get_sizes_progress, throughput_bps, usize_u64,
};
use crate::secret::get_or_create_secret;
use crate::store::TempStore;
use crate::ticket::print_hash;

struct RecvStats {
    total_files: u64,
    payload_size: u64,
    stats: Stats,
}

pub(crate) async fn run(args: ReceiveArgs) -> Result<(), AppError> {
    let secret = get_or_create_secret(args.common.verbose > 0)?;
    let cwd = std::env::current_dir().context("current directory")?;
    let endpoint = bind_recv_endpoint(secret, &args.common, &args.ticket).await?;
    let hash = args.ticket.hash();
    let mut temp = TempStore::open_recv(&cwd, &hash).await?;
    let result = run_recv_interruptible(&endpoint, &temp, &args).await;
    endpoint.close().await;
    drop(temp.close().await);
    match result {
        Ok(recv) => {
            print_recv_stats(&args, &recv);
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn run_recv_interruptible(
    endpoint: &Endpoint,
    temp: &TempStore,
    args: &ReceiveArgs,
) -> Result<RecvStats, AppError> {
    tokio::select! {
        result = recv_body(endpoint, temp, args) => result,
        _ = tokio::signal::ctrl_c() => Err(AppError::Interrupted),
    }
}

async fn recv_body(
    endpoint: &Endpoint,
    temp: &TempStore,
    args: &ReceiveArgs,
) -> Result<RecvStats, AppError> {
    let mp = MultiProgress::new();
    set_draw_target(&mp, args.common.no_progress);
    let db = temp.blobs()?;
    let hash_and_format = args.ticket.hash_and_format();
    trace!("computing local");
    let local = db
        .remote()
        .local(hash_and_format)
        .await
        .map_err(anyhow::Error::from)?;
    trace!("local done");
    let (stats, total_files, payload_size) = if local.is_complete() {
        already_complete(&local, hash_and_format)?
    } else {
        download_missing(endpoint, db, args, &local, &mp).await?
    };
    let collection = Collection::load(hash_and_format.hash, db.as_ref())
        .await
        .map_err(anyhow::Error::from)?;
    announce_export(&collection, args);
    export_collection(db, collection, &mp).await?;
    Ok(RecvStats {
        total_files,
        payload_size,
        stats,
    })
}

fn already_complete(
    local: &LocalInfo,
    hash_and_format: HashAndFormat,
) -> anyhow::Result<(Stats, u64, u64)> {
    println!("{} already complete", hash_and_format.hash);
    let total_files = local
        .children()
        .context("complete local set missing child count")?
        .saturating_sub(1);
    Ok((Stats::default(), total_files, 0))
}

fn announce_export(collection: &Collection, args: &ReceiveArgs) {
    if args.common.verbose > 1 {
        for (name, hash) in collection.iter() {
            println!("    {} {name}", print_hash(hash, args.common.format));
        }
    }
    if let Some((name, _)) = collection.iter().next()
        && let Some(first) = name.split('/').next()
    {
        println!("exporting to {first}");
    }
}

fn print_recv_stats(args: &ReceiveArgs, recv: &RecvStats) {
    if args.common.verbose == 0 {
        return;
    }
    println!(
        "downloaded {} files, {}. took {} ({}/s)",
        recv.total_files,
        HumanBytes(recv.payload_size),
        HumanDuration(recv.stats.elapsed),
        HumanBytes(throughput_bps(
            recv.stats.total_bytes_read(),
            recv.stats.elapsed
        )),
    );
}

async fn download_missing(
    endpoint: &Endpoint,
    db: &Store,
    args: &ReceiveArgs,
    local: &LocalInfo,
    mp: &MultiProgress,
) -> anyhow::Result<(Stats, u64, u64)> {
    let hash_and_format = args.ticket.hash_and_format();
    trace!("{} not complete", hash_and_format.hash);
    let cp = take_connect_progress(mp);
    let connection = endpoint
        .connect(args.ticket.addr().clone(), iroh_blobs::protocol::ALPN)
        .await
        .context("connect to sender")?;
    cp.finish_and_clear();
    let sp = take_get_sizes_progress(mp);
    let (_hash_seq, sizes) =
        get_hash_seq_and_sizes(&connection, &hash_and_format.hash, 1024 * 1024 * 32, None)
            .await
            .map_err(show_get_error)?;
    sp.finish_and_clear();
    let total_size = sizes.iter().copied().sum::<u64>();
    let payload_size = sizes.iter().skip(2).copied().sum::<u64>();
    let total_files = usize_u64(sizes.len().saturating_sub(1));
    eprintln!(
        "getting collection {} {} files, {}",
        print_hash(&args.ticket.hash(), args.common.format),
        total_files,
        HumanBytes(payload_size)
    );
    if args.common.verbose > 0 {
        eprintln!(
            "getting {} blobs in total, {}",
            total_files + 1,
            HumanBytes(total_size)
        );
    }
    let stats = run_get_stream(db, connection, local, mp.clone(), total_size).await?;
    Ok((stats, total_files, payload_size))
}

async fn run_get_stream(
    db: &Store,
    connection: iroh::endpoint::Connection,
    local: &LocalInfo,
    mp: MultiProgress,
    total_size: u64,
) -> anyhow::Result<Stats> {
    let (tx, rx) = mpsc::channel(32);
    let local_size = local.local_bytes();
    let get = db.remote().execute_get(connection, local.missing());
    let task = tokio::spawn(show_download_progress(mp, rx, local_size, total_size));
    let mut stats = Stats::default();
    let mut stream = get.stream();
    while let Some(item) = stream.next().await {
        trace!("got item {item:?}");
        if let Some(value) = on_get_progress_item(item, &tx).await? {
            stats = value;
            break;
        }
    }
    drop(tx);
    task.await.ok();
    Ok(stats)
}
