//! CLAUDE (claude-opus-5-5): textures load small first, then sharp, from the per-GPU texture sets
//! `tools/ktx2_convert` writes; jpg/png images get mipmaps. Native and web.
//!
//! - **Set**: `<sets>/index.json` lists the sets in order of preference; the first this GPU can sample is used
//!   (`bc`: BC7/BC5, `astc4x4`, `orig`: jpg/png). `CORN_TEXTURES=<set>|none` (native) or `?textures=<set>|none`
//!   (web) picks one; `none` loads every image as it is.
//! - **Low**: the default asset source's reader is wrapped ([`TextureLoadingPlugin::register_source`]): a read of
//!   a texture the manifest lists (as its asset path, like `textures/wood.jpg`) gets its low file, at most
//!   `split` (512) wide. The original is never read, unless there's no set, the manifest maps it to the original,
//!   or it isn't listed.
//! - **High**: once every low file is in, the high files load, a few at a time, smallest first, and replace the
//!   low images under the same handles. A KTX2 high file holds only the levels above the low file's.
//!   `CORN_TEXTURES_HIGH=off` (native) or `?high=off` (web) skips them, to compare.
//! - **Mips**: jpg/png images, listed or not, get a full chain at load (KTX2 files carry theirs).
//! - **Seeing it**: info logs say which set, and at the end of each phase how many textures, how many bytes,
//!   how long, and which images were loaded from their own path. The [`TextureLoading`] resource has the same,
//!   live (reflected, for scripts and the remote console).
//!
//! ```ignore
//! let textures = TextureLoadingPlugin::new("assets");
//! textures.register_source(&mut app); // before DefaultPlugins (AssetPlugin)
//! app.add_plugins(DefaultPlugins).add_plugins(textures);
//! ```

mod high;
mod loader;
pub mod manifest;
mod mips;
mod reader;
mod state;

use std::sync::Arc;

use bevy::{
    asset::io::{AssetSource, AssetSourceBuilder, AssetSourceId, ErasedAssetReader},
    image::{CompressedImageFormatSupport, CompressedImageFormats},
    log::{info, warn},
    platform::time::Instant,
    prelude::*,
    tasks::block_on,
};

use crate::state::{mib, Shared};
pub use loader::TextureLoadError;

pub struct TextureLoadingPlugin {
    shared: Arc<Shared>,
    asset_dir: String,
}

impl TextureLoadingPlugin {
    /// `asset_dir`: the default source's folder (`AssetPlugin::file_path`); the sets are in its `texture_sets`
    pub fn new(asset_dir: impl Into<String>) -> Self {
        Self::with_sets(asset_dir, "texture_sets")
    }

    /// `sets_dir`: the sets' folder, relative to the assets root
    pub fn with_sets(asset_dir: impl Into<String>, sets_dir: impl Into<String>) -> Self {
        let shared = Shared::new(sets_dir.into(), requested(), high_wanted());
        Self { shared: Arc::new(shared), asset_dir: asset_dir.into() }
    }

    /// wrap the default asset source's reader. Call before adding `AssetPlugin` (`DefaultPlugins`)
    pub fn register_source(&self, app: &mut App) {
        let shared = self.shared.clone();
        let mut make = AssetSource::get_default_reader(self.asset_dir.clone());
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || {
                let inner: Arc<dyn ErasedAssetReader> = Arc::from(make());
                let _ = shared.inner.set(inner.clone());
                Box::new(reader::ManifestReader { inner, shared: shared.clone() })
            }),
        );
    }
}

/// the set asked for, and by what
fn requested() -> (String, &'static str) {
    #[cfg(target_arch = "wasm32")]
    {
        let asked = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
            .and_then(|p| p.get("textures"));
        if let Some(set) = asked {
            return (set, "?textures=");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(set) = std::env::var("CORN_TEXTURES") {
        return (set, "CORN_TEXTURES");
    }
    ("auto".into(), "default")
}

/// false when `?high=off` / `CORN_TEXTURES_HIGH=off` (or 0, no, false)
fn high_wanted() -> bool {
    #[cfg(target_arch = "wasm32")]
    let asked = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
        .and_then(|p| p.get("high"));
    #[cfg(not(target_arch = "wasm32"))]
    let asked = std::env::var("CORN_TEXTURES_HIGH").ok();
    !matches!(asked.as_deref(), Some("off" | "0" | "no" | "false"))
}

#[derive(Resource)]
pub(crate) struct SharedState(pub Arc<Shared>);

impl Plugin for TextureLoadingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SharedState(self.shared.clone()))
            .init_resource::<TextureLoading>()
            .init_resource::<high::HighQueue>()
            .register_type::<TextureLoading>()
            .add_systems(Update, (track, high::run).chain());
    }

    fn finish(&self, app: &mut App) {
        // CLAUDE: like Bevy's ImageLoader, which this replaces for png/jpg/ktx2: registered later, it's chosen
        let formats = app.world().get_resource::<CompressedImageFormatSupport>().map_or_else(
            || {
                warn!("texture_loading: no CompressedImageFormatSupport (no renderer?); assuming no block compression");
                CompressedImageFormats::NONE
            },
            |r| r.0,
        );
        let _ = block_on(self.shared.gpu.set(formats));
        self.shared.log.lock().unwrap().started = Some(Instant::now());
        if self.shared.inner.get().is_none() {
            warn!("texture_loading: register_source wasn't called before AssetPlugin; images load as they are");
            let _ = block_on(self.shared.manifest.set(None));
        }
        app.register_asset_loader(loader::TextureLoader { formats, shared: self.shared.clone() });
        info!("texture_loading: its loader replaces Bevy's ImageLoader for png/jpg/jpeg/ktx2 (so Bevy warns of a duplicate loader)");
    }
}

/// what's loading, live
#[derive(Resource, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Resource)]
pub struct TextureLoading {
    /// the set in use; "none": images load as they are
    pub set: String,
    pub phase: LoadPhase,
    pub low: Progress,
    pub high: Progress,
    /// the file most recently asked for
    pub current: String,
    /// every manifest texture, once a phase ends: "low KiB + high KiB  WxH, N mips, format  path"
    pub textures: Vec<String>,
    /// images loaded from their own path (not listed, or listed as originals), once a phase ends
    pub direct: Vec<String>,
}

#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoadPhase {
    /// the set isn't chosen yet (nothing has been read)
    #[default]
    Waiting,
    /// low files
    Low,
    /// high files, the low ones are all in
    High,
    Done,
    /// no set: images load as they are
    Off,
}

#[derive(Reflect, Default, Clone, PartialEq, Debug)]
pub struct Progress {
    pub done: u32,
    pub failed: u32,
    /// manifest textures this phase loads
    pub total: u32,
    pub bytes: u64,
    pub total_bytes: u64,
    /// from its first request to its last load
    pub seconds: f32,
}

impl TextureLoading {
    /// every low texture is in (or there's no set): the scene can be shown
    pub fn low_done(&self) -> bool {
        matches!(self.phase, LoadPhase::High | LoadPhase::Done | LoadPhase::Off)
    }
}

/// low done once every texture asked for is in and all the manifest's were asked for, or nothing new was asked
/// for this long (a manifest can list textures the app never uses)
const LOW_QUIET_SECONDS: f32 = 3.0;

// CLAUDE: copy the log into the resource, and move between phases
fn track(shared: Res<SharedState>, mut state: ResMut<TextureLoading>, mut queue: ResMut<high::HighQueue>) {
    let shared = &shared.0;
    let Some(decided) = shared.manifest.get() else { return };
    let Some(manifest) = decided else {
        if state.phase != LoadPhase::Off {
            state.phase = LoadPhase::Off;
            state.set = "none".into();
        }
        return;
    };

    let mut next = state.clone();
    next.set = manifest.set.clone();
    let mut log = shared.log.lock().unwrap();
    let listed: Vec<_> = manifest.images.iter().filter(|(k, e)| e.redirects(k)).map(|(_, e)| e).collect();
    let span = |p: &state::Phase| match (p.first, p.last) {
        (Some(a), Some(b)) => b.saturating_duration_since(a).as_secs_f32(),
        _ => 0.0,
    };
    next.low = Progress {
        done: log.low.done,
        failed: log.low.failed,
        total: listed.len() as u32,
        bytes: log.low.bytes,
        total_bytes: listed.iter().map(|e| e.low_bytes).sum(),
        seconds: span(&log.low),
    };
    next.high = Progress {
        done: log.high.done,
        failed: log.high.failed,
        total: listed.iter().filter(|e| e.high.is_some()).count() as u32,
        bytes: log.high.bytes,
        total_bytes: listed.iter().map(|e| e.high_bytes).sum(),
        seconds: span(&log.high),
    };
    next.current = log.current.clone();

    let since_start = |t: Option<Instant>| match (log.started, t) {
        (Some(s), Some(t)) => t.saturating_duration_since(s).as_secs_f32(),
        _ => 0.0,
    };
    match next.phase {
        LoadPhase::Waiting | LoadPhase::Off => next.phase = LoadPhase::Low,
        LoadPhase::Low => {
            let p = &log.low;
            let settled = p.done + p.failed >= p.requested;
            let last = [p.last, p.first, log.decided].into_iter().flatten().max();
            let quiet = last.is_some_and(|t| t.elapsed().as_secs_f32() >= LOW_QUIET_SECONDS);
            if settled && (p.requested >= next.low.total || quiet) {
                info!(
                    "texture_loading: low files done: {} textures ({} failed), {} in {:.2} s, {:.2} s after startup (set {:?})",
                    p.done, p.failed, mib(p.bytes), next.low.seconds, since_start(p.last.or(log.decided)), manifest.set,
                );
                if !shared.high {
                    info!("texture_loading: high files skipped (CORN_TEXTURES_HIGH / ?high=off)");
                    report(&mut log, &mut next, true);
                    next.phase = LoadPhase::Done;
                    drop(log);
                    state.set_if_neq(next);
                    return;
                }
                report(&mut log, &mut next, false);
                next.phase = LoadPhase::High;
                drop(log);
                high::start(&mut queue, shared);
                state.set_if_neq(next);
                return;
            }
        }
        LoadPhase::High => {
            if queue.idle() {
                let p = &log.high;
                info!(
                    "texture_loading: high files done: {} textures ({} failed), {} in {:.2} s, {:.2} s after startup",
                    p.done, p.failed, mib(p.bytes), next.high.seconds, since_start(p.last.or(log.decided)),
                );
                report(&mut log, &mut next, true);
                next.phase = LoadPhase::Done;
            }
        }
        LoadPhase::Done => {}
    }
    drop(log);
    state.set_if_neq(next);
}

/// list every texture and every image loaded from its own path, in the log and the resource
fn report(log: &mut state::Log, state: &mut TextureLoading, table: bool) {
    let kib = |b: u64| b.div_ceil(1024);
    state.textures = log
        .rows
        .iter()
        .map(|(key, r)| {
            let high = if r.high.is_some() { format!("{:>6} KiB", kib(r.high_bytes)) } else { "         -".into() };
            let sharp = match (r.sharp, r.unused) {
                (true, _) => "",
                (false, true) => "  (low only: no material uses it)",
                (false, false) => "  (low only)",
            };
            let again = if r.requests > 1 { format!("  (read {} times)", r.requests) } else { String::new() };
            format!("{:>5} KiB + {high}  {}x{}, {} mips, {}  {key}{sharp}{again}", kib(r.low_bytes), r.size.0, r.size.1, r.mips, r.format)
        })
        .collect();
    state.direct = log.direct.iter().map(|(k, b)| format!("{:>6} KiB  {k}", kib(*b))).collect();
    if table {
        info!("texture_loading: textures (low + high, on the GPU now):\n  {}", state.textures.join("\n  "));
    }
    if !state.direct.is_empty() {
        info!(
            "texture_loading: {} images loaded from their own path (not in the manifest, or listed as originals):\n  {}",
            state.direct.len(),
            state.direct.join("\n  "),
        );
    }
}
