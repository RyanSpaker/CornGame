//! CLAUDE (claude-opus-5-5): what the reader, the loader and the systems share: the chosen set's manifest (read
//! once, on the first asset read, after the GPU's formats are known) and a log of every texture load.

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    asset::io::{AssetReaderError, ErasedAssetReader},
    image::{CompressedImageFormats, Image, ImageFormat, ImageType},
    log::{info, warn},
    platform::time::Instant,
};

use crate::manifest::{Entry, Index, Manifest};

pub(crate) const KTX2_MAGIC: [u8; 12] = [0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A];

pub(crate) struct Shared {
    /// folder of the sets, relative to the assets root
    pub sets_dir: String,
    /// "auto", "none", or a set's name; and where that came from, for the log
    pub requested: (String, &'static str),
    /// load the high files (off: the low ones stay, to compare)
    pub high: bool,
    /// the GPU's block-compressed formats, set in `finish`
    pub gpu: async_lock::OnceCell<CompressedImageFormats>,
    /// the default source's own reader: high files and originals are read with it, never redirected
    pub inner: OnceLock<Arc<dyn ErasedAssetReader>>,
    /// `None`: no set; everything loads as usual
    pub manifest: async_lock::OnceCell<Option<Arc<Manifest>>>,
    pub log: Mutex<Log>,
}

impl Shared {
    pub fn new(sets_dir: String, requested: (String, &'static str), high: bool) -> Self {
        Self {
            sets_dir,
            requested,
            high,
            gpu: async_lock::OnceCell::new(),
            inner: OnceLock::new(),
            manifest: async_lock::OnceCell::new(),
            log: Mutex::new(Log::default()),
        }
    }

    /// the chosen set's manifest, reading it on first use
    pub async fn manifest(&self, inner: &dyn ErasedAssetReader) -> Option<Arc<Manifest>> {
        self.manifest
            .get_or_init(|| async {
                let gpu = *self.gpu.wait().await;
                let chosen = self.choose(inner, gpu).await;
                self.log.lock().unwrap().decided = Some(Instant::now());
                chosen
            })
            .await
            .clone()
    }

    /// the manifest, if it has been read
    pub fn manifest_now(&self) -> Option<Arc<Manifest>> {
        self.manifest.get().cloned().flatten()
    }

    pub fn entry(&self, key: &str) -> Option<Entry> {
        self.manifest_now()?.images.get(key).cloned()
    }

    async fn choose(&self, inner: &dyn ErasedAssetReader, gpu: CompressedImageFormats) -> Option<Arc<Manifest>> {
        let (requested, from) = (&self.requested.0, self.requested.1);
        if requested == "none" {
            info!("texture_loading: no texture set ({from}); images load as they are, jpg/png get mips");
            return None;
        }
        let index_path = format!("{}/index.json", self.sets_dir);
        let index: Index = match read_json(inner, &index_path).await {
            Ok(index) => index,
            Err(e) => {
                info!("texture_loading: no texture sets ({index_path}: {e}); images load as they are, jpg/png get mips");
                return None;
            }
        };
        let named = (requested != "auto").then(|| index.sets.iter().find(|s| &s.name == requested)).flatten();
        let set = match named {
            Some(set) if set.supported(gpu) => set,
            _ => {
                if requested != "auto" {
                    warn!("texture_loading: set {requested:?} ({from}) is not in {index_path} or this GPU can't sample it; choosing one");
                }
                let Some(set) = index.sets.iter().find(|s| s.supported(gpu)) else {
                    warn!("texture_loading: no set in {index_path} works on this GPU ({gpu:?}); images load as they are");
                    return None;
                };
                set
            }
        };
        let path = format!("{}/{}/manifest.json", self.sets_dir, set.name);
        let manifest: Manifest = match read_json(inner, &path).await {
            Ok(m) => m,
            Err(e) => {
                warn!("texture_loading: {path}: {e}; images load as they are");
                return None;
            }
        };
        let (low, high): (u64, u64) = manifest.images.values().fold((0, 0), |(l, h), e| (l + e.low_bytes, h + e.high_bytes));
        let how = if named.is_some_and(|s| s.supported(gpu)) { from } else { "first the GPU supports" };
        info!(
            "texture_loading: set {:?} ({how}; GPU {gpu:?}): {} textures, split at {}, low {}, high {}",
            manifest.set, manifest.images.len(), manifest.split, mib(low), mib(high),
        );
        Some(Arc::new(manifest))
    }
}

async fn read_json<T: serde::de::DeserializeOwned>(inner: &dyn ErasedAssetReader, path: &str) -> Result<T, String> {
    let bytes = read_all(inner, Path::new(path)).await.map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub(crate) async fn read_all(inner: &dyn ErasedAssetReader, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
    let mut reader = inner.read(path).await?;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

/// what the bytes are: KTX2, PNG or JPEG by their magic, else by the path's extension
pub(crate) fn image_type<'a>(bytes: &[u8], path: &'a str) -> ImageType<'a> {
    if bytes.starts_with(&KTX2_MAGIC) {
        ImageType::Format(ImageFormat::Ktx2)
    } else if bytes.starts_with(b"\x89PNG") {
        ImageType::Format(ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ImageType::Format(ImageFormat::Jpeg)
    } else {
        ImageType::Extension(Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or(""))
    }
}

pub(crate) fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// asset path -> manifest key: `/`-separated
pub(crate) fn key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[derive(Default)]
pub(crate) struct Phase {
    pub requested: u32,
    pub done: u32,
    pub failed: u32,
    pub bytes: u64,
    pub first: Option<Instant>,
    pub last: Option<Instant>,
}

/// one manifest texture
pub(crate) struct Row {
    pub low_bytes: u64,
    pub high: Option<String>,
    pub high_bytes: u64,
    /// what the GPU has now
    pub size: (u32, u32),
    pub format: String,
    pub mips: u32,
    pub sharp: bool,
    /// nothing held it when its high file was due (a texture no material uses)
    pub unused: bool,
    /// reads of its path: more than one when its image was dropped and loaded again
    pub requests: u32,
}

#[derive(Default)]
pub(crate) struct Log {
    pub started: Option<Instant>,
    /// when the set (or none) was chosen
    pub decided: Option<Instant>,
    pub low: Phase,
    pub high: Phase,
    pub rows: BTreeMap<String, Row>,
    /// images loaded from their own path: not in the manifest, or it says the original. path -> bytes
    pub direct: BTreeMap<String, u64>,
    /// the file most recently asked for
    pub current: String,
    /// the high phase has started: textures read again since go back in its queue
    pub high_started: bool,
    /// read again during the high phase: their high files are due again
    pub requeue: Vec<String>,
}

impl Log {
    pub fn describe(&mut self, key: &str, image: &Image) {
        if let Some(row) = self.rows.get_mut(key) {
            let d = &image.texture_descriptor;
            row.size = (d.size.width, d.size.height);
            row.format = format!("{:?}", d.format);
            row.mips = d.mip_level_count;
        }
    }
}
