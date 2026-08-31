use std::path::{Path, PathBuf};

use anyhow::Context;
use data_encoding::HEXLOWER;
use iroh_blobs::Hash;
use iroh_blobs::store::fs::FsStore;
use rand::RngExt;

use crate::error::AppError;

pub(crate) struct TempStore {
    dir: PathBuf,
    store: Option<FsStore>,
    closed: bool,
}

impl TempStore {
    pub(crate) async fn create_send(cwd: &Path, source: &Path) -> Result<Self, AppError> {
        let suffix = rand::rng().random::<[u8; 16]>();
        let dir = cwd.join(format!(".gap-send-{}", HEXLOWER.encode(&suffix)));
        if dir.exists() {
            return Err(AppError::Fail(anyhow::anyhow!(
                "can not share twice from the same directory: {}",
                cwd.display()
            )));
        }
        if cwd.join(source) == cwd {
            return Err(AppError::Fail(anyhow::anyhow!(
                "can not share from the current directory"
            )));
        }
        tokio::fs::create_dir_all(&dir)
            .await
            .context("create send blob store directory")?;
        Self::load_into(dir).await
    }

    pub(crate) async fn open_recv(cwd: &Path, hash: &Hash) -> Result<Self, AppError> {
        let dir = cwd.join(format!(".gap-recv-{}", hash.to_hex()));
        tokio::fs::create_dir_all(&dir)
            .await
            .context("create receive blob store directory")?;
        Self::load_into(dir).await
    }

    async fn load_into(dir: PathBuf) -> Result<Self, AppError> {
        let mut temp = Self {
            dir,
            store: None,
            closed: false,
        };
        let store = FsStore::load(&temp.dir)
            .await
            .map_err(anyhow::Error::from)
            .context("load blob store")?;
        temp.store = Some(store);
        Ok(temp)
    }

    pub(crate) fn blobs(&self) -> anyhow::Result<&FsStore> {
        self.store.as_ref().context("TempStore already closed")
    }

    pub(crate) async fn close(&mut self) -> anyhow::Result<()> {
        if let Some(store) = self.store.take() {
            store
                .shutdown()
                .await
                .map_err(anyhow::Error::from)
                .context("shutdown blob store")?;
        }
        if self.dir.exists() {
            tokio::fs::remove_dir_all(&self.dir)
                .await
                .context("remove blob store directory")?;
        }
        self.closed = true;
        Ok(())
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        // Cannot await FsStore::shutdown from Drop.
        drop(std::fs::remove_dir_all(&self.dir));
    }
}
