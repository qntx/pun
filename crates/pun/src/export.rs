use indicatif::MultiProgress;
use iroh_blobs::api::Store;
use iroh_blobs::api::blobs::{ExportMode, ExportOptions, ExportProgressItem};
use iroh_blobs::format::collection::Collection;
use n0_future::StreamExt;

use crate::path::get_export_path;
use crate::progress::{make_export_item_progress, make_export_overall_progress, usize_u64};

pub(crate) async fn export_collection(
    db: &Store,
    collection: Collection,
    mp: &MultiProgress,
) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let op = mp.add(make_export_overall_progress());
    op.set_length(usize_u64(collection.len()));
    for (i, (name, hash)) in collection.iter().enumerate() {
        op.set_position(usize_u64(i));
        let target = get_export_path(&root, name)?;
        if target.exists() {
            eprintln!(
                "target {} already exists. Export stopped.",
                target.display()
            );
            eprintln!(
                "You can remove the file or directory and try again. The download will not be repeated."
            );
            anyhow::bail!("target {} already exists", target.display());
        }
        let mut stream = db
            .export_with_opts(ExportOptions {
                hash: *hash,
                target,
                mode: ExportMode::Copy,
            })
            .stream()
            .await;
        let pb = mp.add(make_export_item_progress());
        pb.set_message(format!("exporting {name}"));
        drain_export_stream(&mut stream, &pb, name).await?;
    }
    op.finish_and_clear();
    Ok(())
}

async fn drain_export_stream<S>(
    stream: &mut S,
    pb: &indicatif::ProgressBar,
    name: &str,
) -> anyhow::Result<()>
where
    S: StreamExt<Item = ExportProgressItem> + Unpin,
{
    while let Some(item) = stream.next().await {
        match item {
            ExportProgressItem::Size(size) => {
                pb.set_length(size);
            }
            ExportProgressItem::CopyProgress(offset) => {
                pb.set_position(offset);
            }
            ExportProgressItem::Done => {
                pb.finish_and_clear();
            }
            ExportProgressItem::Error(cause) => {
                pb.finish_and_clear();
                anyhow::bail!("error exporting {name}: {cause}");
            }
        }
    }
    Ok(())
}
