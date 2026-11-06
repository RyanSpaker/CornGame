//! CLAUDE (claude-opus-5-5): the default asset source's reader, wrapped: a read of a manifest texture's path gets
//! its low file instead, so the original is never fetched (on the web, never downloaded). Everything else, and
//! textures the manifest maps to their originals, passes through.

use std::{path::Path, sync::Arc};

use bevy::{
    asset::io::{AssetReader, AssetReaderError, ErasedAssetReader, PathStream, Reader, VecReader},
    platform::time::Instant,
};

use crate::state::{key, read_all, Row, Shared};

pub(crate) struct ManifestReader {
    pub inner: Arc<dyn ErasedAssetReader>,
    pub shared: Arc<Shared>,
}

impl AssetReader for ManifestReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let key = key(path);
        let manifest = self.shared.manifest(&*self.inner).await;
        let Some(entry) = manifest.as_ref().and_then(|m| m.images.get(&key)).filter(|e| e.redirects(&key)) else {
            return self.inner.read(path).await;
        };
        {
            let now = Instant::now();
            let mut log = self.shared.log.lock().unwrap();
            log.low.requested += 1;
            log.low.first.get_or_insert(now);
            log.current = entry.low.clone();
            // CLAUDE: read again (its image was dropped, now wanted again): the low file comes back, so the high one
            // is due again
            let row = log.rows.entry(key.clone()).or_insert_with(|| Row {
                low_bytes: entry.low_bytes,
                high: entry.high.clone(),
                high_bytes: entry.high_bytes,
                size: (0, 0),
                format: String::new(),
                mips: 0,
                sharp: entry.high.is_none(),
                unused: false,
                requests: 0,
            });
            row.requests += 1;
            if row.requests > 1 && entry.high.is_some() {
                row.sharp = false;
                row.unused = false;
                if log.high_started {
                    log.requeue.push(key.clone());
                }
            }
        }
        match read_all(&*self.inner, Path::new(&entry.low)).await {
            Ok(bytes) => Ok(Box::new(VecReader::new(bytes))),
            Err(e) => {
                // CLAUDE: counted, so the low phase doesn't wait for it forever
                let mut log = self.shared.log.lock().unwrap();
                log.low.failed += 1;
                bevy::log::warn!("texture_loading: {key}: its low file {} failed: {e}", entry.low);
                Err(e)
            }
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        // CLAUDE: a redirected texture's .meta would describe the original; the low file has none
        let key = key(path);
        let original = key.strip_suffix(".meta").unwrap_or(&key);
        let manifest = self.shared.manifest(&*self.inner).await;
        if manifest.and_then(|m| m.images.get(original).map(|e| e.redirects(original))).unwrap_or(false) {
            return Err(AssetReaderError::NotFound(path.to_path_buf()));
        }
        self.inner.read_meta(path).await
    }

    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> {
        self.inner.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.inner.is_directory(path).await
    }
}
