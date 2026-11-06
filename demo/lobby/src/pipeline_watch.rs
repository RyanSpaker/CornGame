//! CLAUDE (claude-sonnet-5-5) 2026-11-05: warnings for render pipelines that were not precompiled.
//!
//! WHY: the lobby builds its pipelines during loading. One first needed mid-game stalls: on wasm (one
//! thread) `PipelineCache` compiles it synchronously inside the frame, so the browser freezes for as long
//! as shader translation plus the driver's program link take; on native the build is async, so nothing
//! freezes but the draws using it are silently skipped until it is ready (pop-in).
//!
//! Two separate detectors, both silent until the game *arms* them (see [`PipelineWatchArm`]; the lobby's
//! policy is in pipeline_watch_arm.rs) *and* precompile.rs has caught up (everything requested is built for
//! a known view setup, and ready). Before that, building pipelines is expected loading work: precompile
//! only learns a camera's setup the frame the camera first renders, so the first view's pipelines are built
//! in that very frame. Anything needed after both is a miss, including what precompile was never asked for.
//! 1. NEEDED: at the start of the render stage every render phase (opaque, alpha mask, transparent,
//!    transmissive, prepass, deferred, shadow) holds the pipeline id each queued draw will use. A draw whose
//!    pipeline is not `Ok` yet was requested this very frame (or is still building). Pipelines requested
//!    earlier for any reason (staggered warm-up, hidden entities) are `Ok` by the time they are drawn and
//!    never warn; pipelines requested but never drawn never appear in a phase.
//! 2. STUTTER: a frame took more than [`LATE_FACTOR`]x the typical frame time and pipelines were built in
//!    it (or the frame before: WebGL2 may defer the driver's link to the first draw). The build is timed
//!    by running `PipelineCache::process_queue` ourselves just before bevy would (its docs allow this;
//!    bevy's own later call then only retries), and that time is printed beside the excess. Only
//!    meaningful on wasm/macOS where the build is synchronous.
//!
//! Detection only: nothing here changes what gets built or when. A stall found here is fixed by drawing the
//! material once during loading (or otherwise requesting its pipelines early), not by touching this file.
use std::{
    collections::VecDeque,
    sync::{atomic::{AtomicBool, Ordering}, Arc},
};

use bevy::{
    core_pipeline::{
        core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
        deferred::{AlphaMask3dDeferred, Opaque3dDeferred},
        prepass::{AlphaMask3dPrepass, Opaque3dPrepass},
    },
    log::{debug, warn},
    pbr::{Shadow, Transmissive3d},
    platform::{collections::HashSet, time::Instant},
    prelude::*,
    render::{
        render_phase::{
            BinnedPhaseItem, BinnedRenderPhase, CachedRenderPipelinePhaseItem, SortedPhaseItem,
            ViewBinnedRenderPhases, ViewSortedRenderPhases,
        },
        render_resource::{CachedPipeline, CachedPipelineState, CachedRenderPipelineId, PipelineCache, PipelineDescriptor},
        Render, RenderApp, RenderSystems,
    },
};

/// CLAUDE: a frame is late when it takes more than this many times the median of the recent frames.
const LATE_FACTOR: f64 = 2.0;
/// CLAUDE: recent frames the typical frame time is taken from; no stutter verdicts before this many exist.
const WINDOW: usize = 60;
/// CLAUDE: render frames, after the game arms the watcher, that precompile gets to catch up. A timeout, not a
/// measure of anything: past it the watcher warns that precompile isn't done and starts listening anyway
/// (its misses are then real or point at what precompile is stuck on). Slow loads may need it raised.
const STARTUP_FRAMES: u32 = 120;
/// CLAUDE: frames a needed pipeline may stay unready (native async build) before it is reported again.
const PENDING_FRAMES: u32 = 3;

/// CLAUDE: set once by the game when loading is over (a clone lives in both the main and render world).
/// Until then the detectors stay quiet. The game, not a timer, decides what "loaded" means.
#[derive(Resource, Clone, Default)]
pub struct PipelineWatchArm(Arc<AtomicBool>);

impl PipelineWatchArm {
    pub fn arm(&self) { self.0.store(true, Ordering::Relaxed); }
    pub fn armed(&self) -> bool { self.0.load(Ordering::Relaxed) }
}

// CLAUDE: lives in the render world; bookkeeping for the two detectors.
#[derive(Resource, Default)]
struct PipelineWatch {
    /// pipelines (by cache index) already accounted for; the cache only ever appends
    seen: usize,
    /// ids that a queued draw needs and that were not `Ok` when the render stage began (set by `scan`)
    needed_unready: Vec<(CachedRenderPipelineId, Option<bool>)>,
    /// needed-and-unready pipeline index -> frames it has been waiting
    pending: bevy::platform::collections::HashMap<usize, u32>,
    reported: HashSet<usize>,
    reported_pending: HashSet<usize>,
    /// precompile has caught up at least once while armed (latched); detectors speak only after
    listening: bool,
    /// render frames spent armed but waiting for precompile
    waited: u32,
    /// render frames scanned, and when the first was: the startup report's clock
    frames: u32,
    first_frame: Option<Instant>,
    /// every pipeline any draw has needed so far (for the precompiled-but-never-drawn report)
    ever_drawn: HashSet<CachedRenderPipelineId>,
    /// set when precompile finishes: the startup report runs one frame later, on what has been drawn by then
    report_due: Option<StartupReport>,
    last_call: Option<Instant>,
    frame_ms: VecDeque<f64>,
    /// what was built in the previous frame, for stutters whose cost lands a frame late
    built_prev: Vec<String>,
}

// CLAUDE: registers the render-world watcher and the arm switch. Render-world only; absent (headless/tests) = nothing.
pub fn plugin(app: &mut App) {
    let arm = PipelineWatchArm::default();
    app.insert_resource(arm.clone());
    let Some(render) = app.get_sub_app_mut(RenderApp) else { return };
    render.insert_resource(arm).init_resource::<PipelineWatch>()
        // after Queue (specialization requests pipelines, fills the phases) and before bevy's own process_queue in Render
        .add_systems(Render, (scan, watch).chain().in_set(RenderSystems::PrepareBindGroups));
}

// CLAUDE: every pipeline id a binned phase will draw with. `key` pulls the id out of that phase's batch-set key.
fn binned<BPI: BinnedPhaseItem>(world: &World, key: impl Fn(&BPI::BatchSetKey) -> CachedRenderPipelineId, out: &mut HashSet<CachedRenderPipelineId>) {
    let Some(phases) = world.get_resource::<ViewBinnedRenderPhases<BPI>>() else { return };
    for phase in phases.values() {
        let phase: &BinnedRenderPhase<BPI> = phase;
        out.extend(phase.multidrawable_meshes.keys().map(&key));
        out.extend(phase.batchable_meshes.keys().map(|(set, _)| key(set)));
        out.extend(phase.unbatchable_meshes.keys().map(|(set, _)| key(set)));
        out.extend(phase.non_mesh_items.keys().map(|(set, _)| key(set)));
    }
}

// CLAUDE: every pipeline id a sorted phase will draw with.
fn sorted<I: SortedPhaseItem + CachedRenderPipelinePhaseItem>(world: &World, out: &mut HashSet<CachedRenderPipelineId>) {
    let Some(phases) = world.get_resource::<ViewSortedRenderPhases<I>>() else { return };
    for phase in phases.values() {
        out.extend(phase.items.values().map(|item| item.cached_pipeline()));
    }
}

// CLAUDE: what was true the frame precompile finished; the report itself waits one frame (see `startup_report`)
struct StartupReport {
    outcome: &'static str,
    /// render frames and seconds from the first render frame to precompile finishing
    frames: u32,
    seconds: f64,
    /// frames since the game armed the watcher
    waited: u32,
    /// scan frame it finished on: the report runs on the next one
    at: u32,
}

// CLAUDE: one line, one frame after precompile finishes: what precompile built, how long it took, and how much of
// it has been drawn so far (every pipeline any draw has needed since the first frame, so the extra frame lets the
// first draws after the build count). Still a floor on what is needed: pipelines for views not yet seen are not in it.
fn startup_report(world: &World, report: &StartupReport) {
    let watch = world.resource::<PipelineWatch>();
    let made = crate::precompile::made(world);
    let made_count = made.map_or(0, |m| m.len());
    let used = made.map_or(0, |m| m.iter().filter(|id| watch.ever_drawn.contains(*id)).count());
    let unprecompiled = watch.ever_drawn.iter().filter(|id| !made.is_some_and(|m| m.contains(*id))).count();
    info!("pipeline_watch: precompile {}: {made_count} pipelines precompiled at startup in {} render frames / {:.2} s \
        since the first frame ({} frames since the game armed the watcher); {used} of them used so far \
        ({} pipelines drawn so far in all, {unprecompiled} of those not precompiled)",
        report.outcome, report.frames, report.seconds, report.waited, watch.ever_drawn.len());
}

// CLAUDE: detector 1, first half: which pipelines do this frame's queued draws need, and which of those are not built yet.
fn scan(world: &mut World) {
    {
        let mut watch = world.resource_mut::<PipelineWatch>();
        watch.frames += 1;
        watch.first_frame.get_or_insert_with(Instant::now);
    }
    let mut needed = HashSet::default();
    binned::<Opaque3d>(world, |k| k.pipeline, &mut needed);
    binned::<AlphaMask3d>(world, |k| k.pipeline, &mut needed);
    binned::<Opaque3dPrepass>(world, |k| k.pipeline, &mut needed);
    binned::<AlphaMask3dPrepass>(world, |k| k.pipeline, &mut needed);
    binned::<Opaque3dDeferred>(world, |k| k.pipeline, &mut needed);
    binned::<AlphaMask3dDeferred>(world, |k| k.pipeline, &mut needed);
    binned::<Shadow>(world, |k| k.pipeline, &mut needed);
    sorted::<Transparent3d>(world, &mut needed);
    sorted::<Transmissive3d>(world, &mut needed);

    world.resource_mut::<PipelineWatch>().ever_drawn.extend(needed.iter().copied());
    // the startup report, a frame after precompile finished
    let due = world.resource::<PipelineWatch>().report_due.as_ref().is_some_and(|r| world.resource::<PipelineWatch>().frames > r.at);
    if due {
        let report = world.resource_mut::<PipelineWatch>().report_due.take().unwrap();
        startup_report(world, &report);
    }

    let armed = world.get_resource::<PipelineWatchArm>().is_some_and(|a| a.armed());
    if armed && !world.resource::<PipelineWatch>().listening {
        let caught_up = crate::precompile::caught_up(world).unwrap_or(true);
        let mut watch = world.resource_mut::<PipelineWatch>();
        watch.waited += 1;
        if caught_up {
            watch.listening = true;
            watch.report_due = Some(StartupReport { outcome: "caught up", frames: watch.frames, waited: watch.waited,
                seconds: watch.first_frame.map_or(0.0, |t| t.elapsed().as_secs_f64()), at: watch.frames });
        } else if watch.waited >= STARTUP_FRAMES {
            watch.listening = true;
            let progress = crate::precompile::progress(world).unwrap_or_default();
            warn!("precompile has not caught up {STARTUP_FRAMES} frames after the game armed pipeline_watch ({progress}); listening anyway, so later warnings may be unfinished loading");
            let mut watch = world.resource_mut::<PipelineWatch>();
            watch.report_due = Some(StartupReport { outcome: "gave up waiting", frames: watch.frames, waited: watch.waited,
                seconds: watch.first_frame.map_or(0.0, |t| t.elapsed().as_secs_f64()), at: watch.frames });
        }
    }
    let Some(cache) = world.get_resource::<PipelineCache>() else { return };
    // each tagged with whether precompile.rs queued it (it did: the pipeline exists but was built this frame)
    let unready: Vec<_> = needed.into_iter()
        .filter(|&id| !matches!(cache.get_render_pipeline_state(id), CachedPipelineState::Ok(_)))
        .map(|id| (id, crate::precompile::built_here(world, id)))
        .collect();
    world.resource_mut::<PipelineWatch>().needed_unready = unready;
}

// CLAUDE: one-line description that tells a pickup variant from a placement variant: label, shader files and defs.
fn describe(cached: &CachedPipeline) -> String {
    let defs = |defs: &[bevy::shader::ShaderDefVal]| defs.iter().map(|d| match d {
        bevy::shader::ShaderDefVal::Bool(name, true) => name.to_string(),
        bevy::shader::ShaderDefVal::Bool(name, false) => format!("!{name}"),
        bevy::shader::ShaderDefVal::Int(name, v) => format!("{name}={v}"),
        bevy::shader::ShaderDefVal::UInt(name, v) => format!("{name}={v}"),
    }).collect::<Vec<_>>().join(",");
    let shader = |h: &Handle<Shader>| h.path().map(|p| p.to_string()).unwrap_or_else(|| format!("{:?}", h.id()));
    match &cached.descriptor {
        PipelineDescriptor::RenderPipelineDescriptor(d) => format!(
            "{:?} vertex={} fragment={} msaa={} blend={} defs=[{}]",
            d.label.as_deref().unwrap_or("unlabeled"),
            shader(&d.vertex.shader),
            d.fragment.as_ref().map_or("none".into(), |f| shader(&f.shader)),
            d.multisample.count,
            d.fragment.as_ref().is_some_and(|f| f.targets.iter().flatten().any(|t| t.blend.is_some())),
            d.fragment.as_ref().map_or(String::new(), |f| defs(&f.shader_defs)),
        ),
        PipelineDescriptor::ComputePipelineDescriptor(d) => format!(
            "compute {:?} shader={} defs=[{}]", d.label.as_deref().unwrap_or("unlabeled"), shader(&d.shader), defs(&d.shader_defs)),
        PipelineDescriptor::MeshPipelineDescriptor(d) => format!("mesh {:?}", d.label.as_deref().unwrap_or("unlabeled")),
    }
}

// CLAUDE: median of the recent frame times (ms)
fn median(samples: &VecDeque<f64>) -> f64 {
    let mut sorted: Vec<f64> = samples.iter().copied().collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted[sorted.len() / 2]
}

// CLAUDE: runs once per render frame. Builds the queue itself (timed), then reports both detectors.
fn watch(cache: Option<ResMut<PipelineCache>>, mut watch: ResMut<PipelineWatch>, arm: Res<PipelineWatchArm>) {
    let Some(mut cache) = cache else { return };
    let armed = arm.armed() && watch.listening;
    let waiting_before: Vec<usize> = cache.waiting_pipelines().collect();

    let started = Instant::now();
    let frame_ms = watch.last_call.map(|last| started.duration_since(last).as_secs_f64() * 1000.0);
    watch.last_call = Some(started);
    cache.process_queue();
    let build_ms = started.elapsed().as_secs_f64() * 1000.0;

    // requested since last frame (appended by Queue-stage specialization and by bevy's own previous call)
    let total = cache.pipelines().count();
    let requested = watch.seen..total;
    watch.seen = total;
    // built by this call: new requests plus retried ones that became ready
    let built: Vec<usize> = requested.clone().chain(waiting_before.into_iter().filter(|id| !requested.contains(id)))
        .filter(|&id| cache.pipelines().nth(id).is_some_and(|p| matches!(p.state, CachedPipelineState::Ok(_)))).collect();
    let built_names: Vec<String> = built.iter().filter_map(|&id| cache.pipelines().nth(id)).map(describe).collect();

    // 1. NEEDED: a queued draw's pipeline was not built when the frame began
    let needed = std::mem::take(&mut watch.needed_unready);
    watch.pending.retain(|id, _| needed.iter().any(|(n, _)| n.id() == *id));
    for (id, precompiled) in needed {
        let index = id.id();
        let Some(pipeline) = cache.pipelines().nth(index) else { continue };
        let ready = matches!(pipeline.state, CachedPipelineState::Ok(_));
        if !armed { continue; }
        if watch.reported.insert(index) {
            let outcome = if ready { format!("built on demand in {build_ms:.1} ms (stalls the frame where the build is synchronous)") }
                else { "still building: its draws are skipped until ready".to_string() };
            let by = match precompiled {
                Some(true) => "precompile built it, but only this frame (spawned or first seen too late)",
                Some(false) => "precompile never requested it",
                None => "no precompile",
            };
            warn!("pipeline needed but not built before the frame [{by}]: {outcome}: {}", describe(pipeline));
        }
        if !ready {
            let age = watch.pending.entry(index).or_insert(0);
            *age += 1;
            let age = *age;
            if age >= PENDING_FRAMES && watch.reported_pending.insert(index) {
                warn!("pipeline still not ready {age} frames after it was needed (draws using it are skipped): {}", describe(pipeline));
            }
        }
    }

    // 2. STUTTER: a late frame, explained by pipelines built in it or the frame before
    if let Some(ms) = frame_ms {
        if armed && watch.frame_ms.len() >= WINDOW {
            let typical = median(&watch.frame_ms);
            if ms > typical * LATE_FACTOR {
                let culprits: Vec<&String> = built_names.iter().chain(watch.built_prev.iter()).collect();
                if culprits.is_empty() {
                    debug!("late frame {ms:.1} ms (typical {typical:.1} ms), no pipeline built: not a compile stall");
                } else {
                    warn!("late frame {ms:.1} ms (typical {typical:.1} ms, +{:.1} ms), pipeline build in this call {build_ms:.1} ms; pipelines built this or the previous frame:\n  {}",
                        ms - typical, culprits.into_iter().cloned().collect::<Vec<_>>().join("\n  "));
                }
            }
        }
        // the late frame itself stays out of the window's median pull by being one sample among many
        watch.frame_ms.push_back(ms);
        if watch.frame_ms.len() > WINDOW { watch.frame_ms.pop_front(); }
    }
    if !built_names.is_empty() { debug!("pipeline build took {build_ms:.1} ms for {} pipeline(s)", built_names.len()); }
    watch.built_prev = built_names;
}
