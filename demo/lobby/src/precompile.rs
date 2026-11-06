//! CLAUDE (claude-sonnet-5-5) 2026-11-05: simple copy of crates/pylon/src/precompile.rs (CLAUDE, claude-opus-5-5):
//! build a mesh's render pipelines when it's spawned, not the first frame a camera sees it.
//!
//! TODO: pylon's precompile, this copy, and pipeline_watch (+ pipeline_watch_arm's policy hook) should be
//! extracted into one crate shared by pylon and the lobby. The two copies will drift (the key logic mirrors
//! bevy_pbr 0.20's specialize systems), so fix them in one place once that exists.
//!
//! Differences from pylon's: no `WarmUpView` (every lobby camera starts active), no `PrecompileSettings`
//! (always automatic) and no `report_misses` (pipeline_watch reports anything a draw needed that wasn't built).
//!
//! Bevy only specializes pipelines for what a view can see. Each `Mesh3d` + `MeshMaterial3d<M>` (for each
//! `M` given to [`register`]) is requested as it is spawned or changes; its main pass, prepass and shadow
//! pipelines are built for every view setup seen so far, once its mesh and material are on the GPU. A view
//! setup seen for the first time gets every pair requested so far.
//!
//! LIMIT: this builds at *spawn*. Something created on the frame it is first needed (the post-it's
//! `PaperMaterial` copy made at pickup, the ink mesh made at placement) is still built that frame. Those need
//! a prototype spawned during loading, or a call to [`PrecompileQueue::request`] ahead of time.
//!
//! The keys are built as `specialize_material_meshes`, `specialize_prepass_material_meshes` and
//! `specialize_shadows` (bevy_pbr 0.20) build them, without what depends on the entity beyond its mesh
//! and material: lightmaps, crossfading visibility ranges, motion vectors of skinned or morphed meshes.
//! A material changed in a way that changes its key isn't rebuilt (pairs are requested once).

use std::any::TypeId;

use bevy::{
    asset::UntypedAssetId,
    core_pipeline::{
        core_3d::Opaque3d,
        deferred::{AlphaMask3dDeferred, Opaque3dDeferred},
        prepass::{AlphaMask3dPrepass, DeferredPrepass, Opaque3dPrepass},
    },
    light::NotShadowCaster,
    material::{
        key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey},
        MaterialProperties, OpaqueRendererMethod, RenderPhaseType,
    },
    mesh::MeshVertexBufferLayoutRef,
    pbr::{
        alpha_mode_pipeline_key, check_views_lights_need_specialization, LightKeyCache, MeshPipelineKey, PreparedMaterial,
        PrepassFragmentShader, PrepassVertexShader, ViewKeyCache, ViewKeyPrepassCache,
    },
    platform::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    prelude::*,
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{CachedPipelineState, CachedRenderPipelineId, PipelineCache, SpecializedMeshPipelineError},
        view::ExtractedView,
        ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
    },
};

// CLAUDE
pub fn plugin(app: &mut App) {
    app.init_resource::<PrecompileQueue>();
    register::<StandardMaterial>(app);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app
        .init_resource::<Precompiler>()
        .add_systems(ExtractSchedule, take_requests)
        // the view key caches are filled in PrepareAssets, the lights' early in Specialize
        .add_systems(Render, precompile.in_set(RenderSystems::Specialize).after(check_views_lights_need_specialization));
}

// CLAUDE
/// `Mesh3d`s with a `MeshMaterial3d<M>` are requested. Call for each material type but StandardMaterial
pub fn register<M: Material>(app: &mut App) {
    // Last: after anything that gives entities meshes (PostUpdate's too), in the frame bevy would first
    // specialize them
    app.add_systems(Last, request_meshes::<M>);
}

// CLAUDE
/// what to build pipelines for: a mesh and its material, and whether it casts shadows (`NotShadowCaster`)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PipelineRequest {
    pub mesh: AssetId<Mesh>,
    pub material: UntypedAssetId,
    pub casts_shadows: bool,
}

// CLAUDE
/// requests, taken by the render world as it extracts. Each pair is built once
#[derive(Resource, Default)]
pub struct PrecompileQueue {
    seen: HashSet<PipelineRequest>,
    new: Vec<PipelineRequest>,
}

impl PrecompileQueue {
    pub fn request<M: Material>(&mut self, mesh: AssetId<Mesh>, material: AssetId<M>, casts_shadows: bool) {
        // a mesh to be filled in later (bevy_rich_text3d's `Mesh3d: default`) is requested once it's real
        if mesh == AssetId::default() {
            return;
        }
        let request = PipelineRequest { mesh, material: material.untyped(), casts_shadows };
        if self.seen.insert(request) {
            self.new.push(request);
        }
    }
}

fn request_meshes<M: Material>(
    mut queue: ResMut<PrecompileQueue>,
    meshes: Query<(&Mesh3d, &MeshMaterial3d<M>, Has<NotShadowCaster>), Or<(Changed<Mesh3d>, Changed<MeshMaterial3d<M>>)>>,
) {
    for (mesh, material, no_shadows) in &meshes {
        queue.request(mesh.id(), material.id(), !no_shadows);
    }
}

// ---------------------------------------------------------------------------- render world

// CLAUDE
#[derive(Resource, Default)]
struct Precompiler {
    /// requested, its mesh or material not on the GPU yet
    waiting: Vec<PipelineRequest>,
    /// built for every view setup seen so far
    ready: Vec<PipelineRequest>,
    /// every view setup seen: the main pass's key, the prepass's (and whether the view defers), the
    /// shadow views'. kept after their views are gone: a camera that sleeps comes back as it was
    main_keys: HashSet<MeshPipelineKey>,
    prepass_keys: HashSet<(MeshPipelineKey, bool)>,
    light_keys: HashSet<MeshPipelineKey>,
    /// every pipeline built here
    made: HashSet<CachedRenderPipelineId>,
    /// as of the last run: nothing waits on assets and at least one view setup is known
    caught_up: bool,
}

fn take_requests(mut main_world: ResMut<MainWorld>, mut precompiler: ResMut<Precompiler>) {
    if let Some(mut queue) = main_world.get_resource_mut::<PrecompileQueue>() {
        precompiler.waiting.append(&mut queue.new);
    }
}

// CLAUDE
/// precompile has done what it was asked: every requested pair is built for a known view setup, and every
/// pipeline it made is ready (not just queued). Pairs requested later can make it false again; pipeline_watch
/// latches the first time. None when the precompiler isn't running (headless)
pub fn caught_up(world: &World) -> Option<bool> {
    let precompiler = world.get_resource::<Precompiler>()?;
    let cache = world.get_resource::<PipelineCache>()?;
    Some(precompiler.caught_up
        && precompiler.made.iter().all(|&id| matches!(cache.get_render_pipeline_state(id), CachedPipelineState::Ok(_))))
}

// CLAUDE
/// every pipeline this precompiler has built, for pipeline_watch's startup report. None when it isn't running
pub fn made(world: &World) -> Option<&HashSet<CachedRenderPipelineId>> {
    world.get_resource::<Precompiler>().map(|p| &p.made)
}

// CLAUDE
/// what precompile is still missing, for the startup timeout's warning. None when it isn't running
pub fn progress(world: &World) -> Option<String> {
    let precompiler = world.get_resource::<Precompiler>()?;
    let cache = world.get_resource::<PipelineCache>()?;
    let unready = precompiler.made.iter().filter(|&&id| !matches!(cache.get_render_pipeline_state(id), CachedPipelineState::Ok(_))).count();
    Some(format!(
        "{} request(s) waiting for their mesh or material on the GPU, {} built pipeline(s) not ready, {} main-pass view setup(s) known{}",
        precompiler.waiting.len(), unready, precompiler.main_keys.len(),
        if precompiler.main_keys.is_empty() { " (no camera has rendered yet)" } else { "" }))
}

// CLAUDE
/// whether this precompiler built `id` (some key/view of some request): pipeline_watch tags its warnings with it.
/// None when the precompiler isn't running (headless)
pub fn built_here(world: &World, id: CachedRenderPipelineId) -> Option<bool> {
    world.get_resource::<Precompiler>().map(|p| p.made.contains(&id))
}

// CLAUDE
/// what a request's pipelines are built from
struct Shape {
    type_id: TypeId,
    mesh_bits: MeshPipelineKey,
    layout: MeshVertexBufferLayoutRef,
    properties: Arc<MaterialProperties>,
    casts_shadows: bool,
}

impl Shape {
    fn of(world: &World, request: &PipelineRequest) -> Option<Self> {
        let mesh = world.get_resource::<RenderAssets<RenderMesh>>()?.get(request.mesh)?;
        let material = world.get_resource::<ErasedRenderAssets<PreparedMaterial>>()?.get(request.material)?;
        Some(Self {
            type_id: request.material.type_id(),
            mesh_bits: MeshPipelineKey::from_bits_retain(mesh.key_bits.bits()),
            layout: mesh.layout.clone(),
            properties: material.properties.clone(),
            casts_shadows: request.casts_shadows,
        })
    }

    fn key(&self, mesh_key: MeshPipelineKey) -> ErasedMaterialPipelineKey {
        ErasedMaterialPipelineKey {
            type_id: self.type_id,
            mesh_key: ErasedMeshPipelineKey::new(mesh_key),
            material_key: self.properties.material_key.clone(),
        }
    }

    /// bevy_pbr's `MaterialPropertiesExt::prepass_reads_material` (crate private)
    fn prepass_reads_material(&self) -> bool {
        self.properties.get_shader(PrepassVertexShader).is_some() || self.properties.get_shader(PrepassFragmentShader).is_some()
    }

    /// as `specialize_material_meshes`
    fn main_pass(&self, world: &mut World, view: MeshPipelineKey) -> Option<Result<CachedRenderPipelineId, SpecializedMeshPipelineError>> {
        let properties = &self.properties;
        let specialize = properties.base_specialize?;
        let material_bits: MeshPipelineKey = properties.mesh_pipeline_key_bits.downcast();
        let key = view | self.mesh_bits | material_bits | alpha_mode_pipeline_key(properties.alpha_mode, &Msaa::from_samples(view.msaa_samples()));
        Some(specialize(world, self.key(key), &self.layout, properties))
    }

    /// as `specialize_prepass_material_meshes`
    fn prepass(&self, world: &mut World, view: MeshPipelineKey, deferred_view: bool) -> Option<Result<CachedRenderPipelineId, SpecializedMeshPipelineError>> {
        let properties = &self.properties;
        let specialize = properties.prepass_specialize?;
        let opaque = matches!(properties.alpha_mode, AlphaMode::Opaque | AlphaMode::AlphaToCoverage | AlphaMode::Mask(_));
        let phase = matches!(properties.render_phase_type, RenderPhaseType::Opaque | RenderPhaseType::AlphaMask);
        if !properties.prepass_enabled || !opaque || !phase || properties.reads_view_transmission_texture {
            return None;
        }
        let mut key = view | self.mesh_bits | alpha_mode_pipeline_key(properties.alpha_mode, &Msaa::from_samples(view.msaa_samples()));
        if deferred_view && matches!(properties.render_method, OpaqueRendererMethod::Deferred) {
            key |= MeshPipelineKey::DEFERRED_PREPASS;
        }
        if self.prepass_reads_material() {
            key |= MeshPipelineKey::PREPASS_READS_MATERIAL;
        }
        Some(specialize(world, &self.key(key), &self.layout, properties))
    }

    /// as `specialize_shadows`
    fn shadow(&self, world: &mut World, light: MeshPipelineKey) -> Option<Result<CachedRenderPipelineId, SpecializedMeshPipelineError>> {
        let properties = &self.properties;
        let specialize = properties.prepass_specialize?;
        if !properties.shadows_enabled || !self.casts_shadows {
            return None;
        }
        let mut key = light | self.mesh_bits;
        if matches!(
            properties.alpha_mode,
            AlphaMode::Mask(_) | AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::AlphaToCoverage
        ) {
            key |= MeshPipelineKey::MAY_DISCARD;
        }
        if self.prepass_reads_material() {
            key |= MeshPipelineKey::PREPASS_READS_MATERIAL;
        }
        Some(specialize(world, &self.key(key), &self.layout, properties))
    }
}

// CLAUDE
/// the view setups there are now
#[derive(Default)]
struct ViewKeys {
    main: Vec<MeshPipelineKey>,
    prepass: Vec<(MeshPipelineKey, bool)>,
    light: Vec<MeshPipelineKey>,
}

impl ViewKeys {
    /// as the passes pick their views: those with the pass's phases
    fn now(world: &mut World) -> Self {
        let deferred: HashMap<_, _> = world.query::<(&ExtractedView, Has<DeferredPrepass>)>().iter(world).map(|(v, d)| (v.retained_view_entity, d)).collect();
        let mut keys = Self::default();
        if let (Some(cache), Some(opaque)) = (world.get_resource::<ViewKeyCache>(), world.get_resource::<ViewBinnedRenderPhases<Opaque3d>>()) {
            keys.main = cache.iter().filter(|(view, _)| opaque.contains_key(*view)).map(|(_, key)| *key).collect();
        }
        if let Some(cache) = world.get_resource::<ViewKeyPrepassCache>() {
            let has = |view| {
                world.get_resource::<ViewBinnedRenderPhases<Opaque3dPrepass>>().is_some_and(|p| p.contains_key(view))
                    || world.get_resource::<ViewBinnedRenderPhases<AlphaMask3dPrepass>>().is_some_and(|p| p.contains_key(view))
                    || world.get_resource::<ViewBinnedRenderPhases<Opaque3dDeferred>>().is_some_and(|p| p.contains_key(view))
                    || world.get_resource::<ViewBinnedRenderPhases<AlphaMask3dDeferred>>().is_some_and(|p| p.contains_key(view))
            };
            keys.prepass = cache.iter().filter(|(view, _)| has(*view)).map(|(view, key)| (*key, deferred.get(view).copied().unwrap_or(false))).collect();
        }
        if let Some(cache) = world.get_resource::<LightKeyCache>() {
            keys.light = cache.values().copied().collect();
        }
        keys
    }
}

// CLAUDE
/// build what's newly ready for every setup seen, and what was ready for setups new this frame
fn precompile(world: &mut World) {
    let now = ViewKeys::now(world);
    world.resource_scope(|world, mut precompiler: Mut<Precompiler>| {
        let precompiler = &mut *precompiler;
        let new = ViewKeys {
            main: now.main.into_iter().filter(|k| precompiler.main_keys.insert(*k)).collect(),
            prepass: now.prepass.into_iter().filter(|k| precompiler.prepass_keys.insert(*k)).collect(),
            light: now.light.into_iter().filter(|k| precompiler.light_keys.insert(*k)).collect(),
        };
        let all = ViewKeys {
            main: precompiler.main_keys.iter().copied().collect(),
            prepass: precompiler.prepass_keys.iter().copied().collect(),
            light: precompiler.light_keys.iter().copied().collect(),
        };

        // ready before: only the new setups. a pair whose assets are gone is dropped
        if !(new.main.is_empty() && new.prepass.is_empty() && new.light.is_empty()) {
            precompiler.ready.retain(|request| {
                let Some(shape) = Shape::of(world, request) else { return false };
                build(world, &shape, &new, &mut precompiler.made);
                true
            });
        }
        // newly ready: every setup
        let mut waiting = std::mem::take(&mut precompiler.waiting);
        waiting.retain(|request| {
            let Some(shape) = Shape::of(world, request) else { return true };
            build(world, &shape, &all, &mut precompiler.made);
            precompiler.ready.push(*request);
            false
        });
        precompiler.waiting = waiting;
        precompiler.caught_up = precompiler.waiting.is_empty() && !precompiler.main_keys.is_empty();
    });
}

fn build(world: &mut World, shape: &Shape, views: &ViewKeys, made: &mut HashSet<CachedRenderPipelineId>) {
    let mut built = Vec::new();
    for &view in &views.main {
        built.extend(shape.main_pass(world, view));
    }
    for &(view, deferred) in &views.prepass {
        built.extend(shape.prepass(world, view, deferred));
    }
    for &light in &views.light {
        built.extend(shape.shadow(world, light));
    }
    for result in built {
        match result {
            Ok(id) => {
                made.insert(id);
            }
            // bevy would hit this too, as the mesh is seen (or the keys here have drifted from bevy's)
            Err(err) => warn!("precompile: {err}"),
        }
    }
}
