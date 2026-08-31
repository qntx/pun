use std::path::{Path, PathBuf};

use anyhow::Context;
use futures_buffered::BufferedStreamExt;
use indicatif::{MultiProgress, ProgressBar};
use iroh_blobs::BlobFormat;
use iroh_blobs::api::blobs::{AddPathOptions, AddProgressItem, ImportMode};
use iroh_blobs::api::{Store, TempTag};
use iroh_blobs::format::collection::Collection;
use n0_future::StreamExt;
use tracing::trace;
use walkdir::WalkDir;

use crate::path::canonicalized_path_to_string;
use crate::progress::{make_import_item_progress, make_import_overall_progress, usize_u64};

fn collect_data_sources(path: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let path = path.canonicalize()?;
    anyhow::ensure!(path.exists(), "path {} does not exist", path.display());
    let root = path.parent().context("context get parent")?.to_path_buf();
    WalkDir::new(path)
        .into_iter()
        .map(|entry| {
            let entry = entry?;
            if !entry.file_type().is_file() {
                return Ok(None);
            }
            let file_path = entry.into_path();
            let relative = file_path.strip_prefix(&root)?;
            let name = canonicalized_path_to_string(relative, true)?;
            anyhow::Ok(Some((name, file_path)))
        })
        .filter_map(Result::transpose)
        .collect()
}

fn on_add_progress_item(
    pb: &ProgressBar,
    name: &str,
    item: AddProgressItem,
    item_size: &mut u64,
) -> anyhow::Result<Option<TempTag>> {
    match item {
        AddProgressItem::Size(size) => {
            *item_size = size;
            pb.set_length(size);
            Ok(None)
        }
        AddProgressItem::CopyProgress(offset) | AddProgressItem::OutboardProgress(offset) => {
            pb.set_position(offset);
            Ok(None)
        }
        AddProgressItem::CopyDone => {
            pb.set_message(format!("computing outboard {name}"));
            pb.set_position(0);
            Ok(None)
        }
        AddProgressItem::Error(cause) => {
            pb.finish_and_clear();
            anyhow::bail!("error importing {name}: {cause}");
        }
        AddProgressItem::Done(tt) => {
            pb.finish_and_clear();
            Ok(Some(tt))
        }
    }
}

async fn import_one_file(
    db: Store,
    name: String,
    path: PathBuf,
    mp: MultiProgress,
) -> anyhow::Result<(String, TempTag, u64)> {
    let pb = mp.add(make_import_item_progress());
    pb.set_message(format!("copying {name}"));
    let mut stream = db
        .add_path_with_opts(AddPathOptions {
            path,
            mode: ImportMode::TryReference,
            format: BlobFormat::Raw,
        })
        .stream()
        .await;
    let mut item_size = 0;
    let temp_tag = loop {
        let item = stream
            .next()
            .await
            .context("import stream ended without a tag")?;
        trace!("importing {name} {item:?}");
        if let Some(tt) = on_add_progress_item(&pb, &name, item, &mut item_size)? {
            break tt;
        }
    };
    Ok((name, temp_tag, item_size))
}

pub(crate) async fn import_paths(
    path: PathBuf,
    db: &Store,
    mp: &MultiProgress,
    jobs: Option<usize>,
) -> anyhow::Result<(TempTag, u64, Collection)> {
    let parallelism = jobs.unwrap_or_else(num_cpus::get);
    let data_sources = collect_data_sources(&path)?;
    let op = mp.add(make_import_overall_progress());
    op.set_message(format!("importing {} files", data_sources.len()));
    op.set_length(usize_u64(data_sources.len()));
    let mut names_and_tags = n0_future::stream::iter(data_sources)
        .map(|(name, file_path)| {
            let db = db.clone();
            let op = op.clone();
            let mp = mp.clone();
            async move {
                op.inc(1);
                import_one_file(db, name, file_path, mp).await
            }
        })
        .buffered_unordered(parallelism)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    op.finish_and_clear();
    names_and_tags.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    let size = names_and_tags.iter().map(|(_, _, size)| *size).sum::<u64>();
    let (collection, tags) = names_and_tags
        .into_iter()
        .map(|(name, tag, _)| ((name, tag.hash()), tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let temp_tag = collection
        .clone()
        .store(db)
        .await
        .map_err(anyhow::Error::from)?;
    drop(tags);
    Ok((temp_tag, size, collection))
}
