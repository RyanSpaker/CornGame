//! CLAUDE (claude-opus-5-5): the high phase. Once every low file is in, each texture with a high file fetches it
//! (a few at a time, smallest first) and its image is replaced under the same handle, so materials pick it up.
//! A KTX2 high file holds only the levels above the low file's; the two are joined into one chain. A jpg/png high
//! file is the full image and gets its own mips.

use std::{collections::VecDeque, path::Path, sync::Arc};

use bevy::{
    asset::RenderAssetUsages,
    image::{ktx2_buffer_to_image, CompressedImageFormats, Image, ImageSampler},
    log::warn,
    platform::time::Instant,
    prelude::*,
    render::render_resource::TextureFormat,
    tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task},
};

use crate::{
    manifest::Entry,
    mips,
    state::{image_type, read_all, Shared, KTX2_MAGIC},
};

/// fetches in flight at once; browsers open about 6 connections per host
const CONCURRENT: usize = 6;

#[derive(Resource, Default)]
pub(crate) struct HighQueue {
    pub started: bool,
    pending: VecDeque<String>,
    running: Vec<(String, AssetId<Image>, Task<Result<(Image, u64), String>>)>,
}

impl HighQueue {
    pub fn idle(&self) -> bool {
        self.started && self.pending.is_empty() && self.running.is_empty()
    }
}

/// what the high file is joined to, from the low image
struct Low {
    /// its levels' data; `None` when the CPU copy is gone (render-world-only assets): read the low file again
    data: Option<Vec<u8>>,
    format: TextureFormat,
    size: (u32, u32),
    mips: u32,
    sampler: ImageSampler,
    usage: RenderAssetUsages,
}

/// queue every texture with a high file, smallest first
pub(crate) fn start(queue: &mut HighQueue, shared: &Shared) {
    let Some(manifest) = shared.manifest_now() else { return };
    let log = shared.log.lock().unwrap();
    let mut keys: Vec<(&String, u64)> = log
        .rows
        .iter()
        .filter(|(_, row)| row.high.is_some() && row.mips > 0)
        .map(|(k, _)| (k, manifest.images.get(k).map_or(0, |e| e.high_bytes)))
        .collect();
    keys.sort_by_key(|(_, bytes)| *bytes);
    queue.pending = keys.into_iter().map(|(k, _)| k.clone()).collect();
    queue.started = true;
    drop(log);
    shared.log.lock().unwrap().high_started = true;
}

pub(crate) fn run(
    mut queue: ResMut<HighQueue>,
    shared: Res<crate::SharedState>,
    server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let shared = &shared.0;
    // CLAUDE: finished: the new image replaces the low one under its handle
    let mut i = 0;
    while i < queue.running.len() {
        let Some(result) = block_on(poll_once(&mut queue.running[i].2)) else {
            i += 1;
            continue;
        };
        let (key, id, _) = queue.running.swap_remove(i);
        let mut log = shared.log.lock().unwrap();
        log.high.last = Some(Instant::now());
        match result {
            Ok((image, bytes)) => {
                log.high.done += 1;
                log.high.bytes += bytes;
                log.describe(&key, &image);
                if let Some(row) = log.rows.get_mut(&key) {
                    row.sharp = true;
                }
                if let Err(e) = images.insert(id, image) {
                    warn!("texture_loading: {key}: its image is gone ({e}); the high file is dropped");
                }
            }
            Err(e) => {
                log.high.failed += 1;
                warn!("texture_loading: {key}: high file failed, keeping the low one: {e}");
            }
        }
    }

    // CLAUDE: read again since the high phase started: due again (once its low image is in)
    let again: Vec<String> = std::mem::take(&mut shared.log.lock().unwrap().requeue);
    queue.pending.extend(again);

    let formats = *shared.gpu.get().unwrap_or(&CompressedImageFormats::NONE);
    let Some(manifest) = shared.manifest_now() else { return };
    let mut tries = queue.pending.len();
    while queue.running.len() < CONCURRENT && tries > 0 {
        tries -= 1;
        let Some(key) = queue.pending.pop_front() else { break };
        let Some(entry) = manifest.images.get(&key).cloned() else { continue };
        let Some(handle) = server.get_handle::<Image>(bevy::asset::AssetPath::from(key.clone())) else {
            // nothing holds it any more: nothing to sharpen
            if let Some(row) = shared.log.lock().unwrap().rows.get_mut(&key) {
                row.unused = true;
            }
            continue;
        };
        let Some(image) = images.get(&handle) else {
            // loaded, but not in Assets yet: next frame
            queue.pending.push_back(key);
            continue;
        };
        let d = &image.texture_descriptor;
        let low = Low {
            data: image.data.clone(),
            format: d.format,
            size: (d.size.width, d.size.height),
            mips: d.mip_level_count,
            sampler: image.sampler.clone(),
            usage: image.asset_usage,
        };
        {
            let mut log = shared.log.lock().unwrap();
            log.high.requested += 1;
            log.high.first.get_or_insert(Instant::now());
            log.current = entry.high.clone().unwrap_or_default();
        }
        let shared = shared.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move { fetch(shared, entry, low, formats).await });
        queue.running.push((key, handle.id(), task));
    }
}

async fn fetch(shared: Arc<Shared>, entry: Entry, low: Low, formats: CompressedImageFormats) -> Result<(Image, u64), String> {
    let inner = shared.inner.get().ok_or("no asset reader")?.clone();
    let path = entry.high.as_deref().ok_or("no high file")?;
    let bytes = read_all(&*inner, Path::new(path)).await.map_err(|e| e.to_string())?;
    let srgb = low.format.is_srgb();
    let image = if bytes.starts_with(&KTX2_MAGIC) {
        let high = ktx2_buffer_to_image(&bytes, formats, srgb).map_err(|e| e.to_string())?;
        let low_data = match low.data.clone() {
            Some(data) => data,
            None => {
                let bytes = read_all(&*inner, Path::new(&entry.low)).await.map_err(|e| e.to_string())?;
                ktx2_buffer_to_image(&bytes, formats, srgb).map_err(|e| e.to_string())?.data.ok_or("empty low file")?
            }
        };
        join(high, low_data, &low)?
    } else {
        let mut image = Image::from_buffer(&bytes, image_type(&bytes, path), formats, srgb, low.sampler.clone(), low.usage)
            .map_err(|e| e.to_string())?;
        mips::generate(&mut image, entry.role);
        image
    };
    Ok((image, bytes.len() as u64))
}

/// the high file's levels, then the low image's: one chain
fn join(mut high: Image, low_data: Vec<u8>, low: &Low) -> Result<Image, String> {
    let d = &high.texture_descriptor;
    let levels = d.mip_level_count;
    let below = ((d.size.width >> levels).max(1), (d.size.height >> levels).max(1));
    if d.format != low.format || below != low.size {
        return Err(format!(
            "high file ({:?} {}x{}, {levels} levels) doesn't continue into the low one ({:?} {}x{})",
            d.format, d.size.width, d.size.height, low.format, low.size.0, low.size.1,
        ));
    }
    let mut data = high.data.take().ok_or("empty high file")?;
    data.extend_from_slice(&low_data);
    high.data = Some(data);
    high.texture_descriptor.mip_level_count = levels + low.mips;
    high.sampler = low.sampler.clone();
    high.asset_usage = low.usage;
    Ok(high)
}
