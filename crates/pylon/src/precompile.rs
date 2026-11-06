//! CLAUDE (claude-opus-5-5): build a mesh's render pipelines when it's spawned, not the first frame a
//! camera sees it. Bevy only specializes pipelines for what a view can see, and compiles them in the
//! background: something first seen is skipped until its pipelines are ready, one pass at a time (a
//! prepass ready before the main pass draws depth with no color: a hole showing the clear color).
//!
//! - [`PrecompileQueue::request`] a (mesh, material) pair: its main pass, prepass and shadow pipelines
//!   are built for every view setup seen so far, once both assets are on the GPU. A view setup seen for
//!   the first time (a camera with other settings) gets every pair requested so far.
//! - With [`PrecompileSettings::auto`] (the default) every `Mesh3d` with a `MeshMaterial3d<M>` is
//!   requested as it's spawned or changes mesh or material, for each `M` [`register`]ed. Without it,
//!   only what's requested is (custom logic decides).
//! - A camera that starts inactive has no view setup until it's woken: give it [`WarmUpView`] and it
//!   renders one (empty) frame first, so its setup is known.
//! - [`PrecompileSettings::report_misses`] (debug builds) warns about pipelines the passes asked for that
//!   weren't precompiled: something not requested, or the key logic here drifting from bevy's.
//!
//! The keys are built as `specialize_material_meshes`, `specialize_prepass_material_meshes` and
//! `specialize_shadows` (bevy_pbr 0.20) build them, without what depends on the entity beyond its mesh
//! and material: lightmaps, crossfading visibility ranges, motion vectors of skinned or morphed meshes.
//! A material changed in a way that changes its key isn't rebuilt (pairs are requested once).

use std::any::TypeId;

use bevy::{
    asset::UntypedAssetId,
    camera::CameraUpdateSystems,
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
        PrepassFragmentShader, PrepassVertexShader, RenderMaterialInstances, RenderMeshInstances,
        SpecializedMaterialPipelineCache, SpecializedPrepassMaterialPipelineCache,
        SpecializedShadowMaterialPipelineCache, ViewKeyCache, ViewKeyPrepassCache,
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
        render_resource::{CachedRenderPipelineId, SpecializedMeshPipelineError},
        sync_world::MainEntity,
        view::ExtractedView,
        ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
    },
};

pub fn plugin(app: &mut App) {
    app.init_resource::<PrecompileSettings>()
        .init_resource::<PrecompileQueue>()
        // scenes spawn it (required by pylon's window views), so it must be reflected
        .register_type::<WarmUpView>()
        // after whatever wakes and sleeps cameras (before CameraUpdateSystems), so this frame it's active
        .add_systems(PostUpdate, warm_up_views.after(CameraUpdateSystems));
    register::<StandardMaterial>(app);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app
        .init_resource::<Precompiler>()
        .add_systems(ExtractSchedule, take_requests)
        .add_systems(
            Render,
            (
                // the view key caches are filled in PrepareAssets, the lights' early in Specialize
                precompile.in_set(RenderSystems::Specialize).after(check_views_lights_need_specialization),
                // after every pass has specialized
                report_misses.in_set(RenderSystems::PhaseSort),
            ),
        );
}

// CLAUDE
/// with [`PrecompileSettings::auto`], `Mesh3d`s with a `MeshMaterial3d<M>` are requested
pub fn register<M: Material>(app: &mut App) {
    // Last: after anything that gives entities meshes (PostUpdate's too), in the frame bevy would first
    // specialize them
    app.add_systems(Last, request_meshes::<M>.run_if(|settings: Res<PrecompileSettings>| settings.auto));
}

// CLAUDE
#[derive(Resource, Clone, Debug)]
pub struct PrecompileSettings {
    /// every mesh is requested as it's spawned, or changes mesh or material. Off, only what's
    /// [`PrecompileQueue::request`]ed is
    pub auto: bool,
    /// warn about pipelines the passes asked for that weren't precompiled. read as it's extracted
    pub report_misses: bool,
}

impl Default for PrecompileSettings {
    fn default() -> Self {
        Self { auto: true, report_misses: cfg!(debug_assertions) }
    }
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

// CLAUDE
/// on an inactive camera: it's woken for one frame (once it has a viewport), so its view setup is known
/// and what's requested is built for it too. It renders nothing that frame (what it sees is worked out
/// for active cameras, before this)
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct WarmUpView;

fn warm_up_views(mut commands: Commands, mut views: Query<(Entity, &mut Camera), With<WarmUpView>>) {
    for (entity, mut camera) in &mut views {
        if camera.physical_viewport_size().is_none() {
            continue;
        }
        camera.is_active = true;
        commands.entity(entity).remove::<WarmUpView>();
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
    /// misses already warned about
    reported: HashSet<CachedRenderPipelineId>,
    report_misses: bool,
}

fn take_requests(mut main_world: ResMut<MainWorld>, mut precompiler: ResMut<Precompiler>) {
    precompiler.report_misses = main_world.get_resource::<PrecompileSettings>().is_some_and(|s| s.report_misses);
    if let Some(mut queue) = main_world.get_resource_mut::<PrecompileQueue>() {
        precompiler.waiting.append(&mut queue.new);
    }
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

// CLAUDE
/// pipelines the passes are using that weren't built here
fn report_misses(
    mut precompiler: ResMut<Precompiler>,
    main: Option<Res<SpecializedMaterialPipelineCache>>,
    prepass: Option<Res<SpecializedPrepassMaterialPipelineCache>>,
    shadow: Option<Res<SpecializedShadowMaterialPipelineCache>>,
    mesh_instances: Option<Res<RenderMeshInstances>>,
    material_instances: Option<Res<RenderMaterialInstances>>,
) {
    let Precompiler { made, reported, report_misses, waiting, ready, .. } = &mut *precompiler;
    if !*report_misses {
        return;
    }
    // why: what the entity's mesh and material are, and how far precompile got with them
    let why = |entity: MainEntity| {
        let mesh = mesh_instances.as_ref().and_then(|m| m.mesh_asset_id(entity));
        let material = material_instances.as_ref().and_then(|m| m.instances.get(&entity)).map(|m| m.asset_id);
        let (Some(mesh), Some(material)) = (mesh, material) else { return "no mesh or material".to_string() };
        let is = |r: &&PipelineRequest| r.mesh == mesh && r.material == material;
        let state = if ready.iter().any(|r| is(&r)) {
            "built, with other keys"
        } else if waiting.iter().any(|r| is(&r)) {
            "requested, its assets not on the GPU yet"
        } else {
            "not requested"
        };
        format!("{state}: mesh {mesh:?}, material {material:?}")
    };
    let main = main.iter().flat_map(|c| c.values()).flat_map(|v| v.iter().map(|(e, id)| ("main pass", *e, *id)));
    let prepass = prepass.iter().flat_map(|c| c.values()).flat_map(|v| v.iter().map(|(e, (_, id, _))| ("prepass", *e, *id)));
    let shadow = shadow.iter().flat_map(|c| c.values()).flat_map(|v| v.iter().map(|(e, (id, _, _))| ("shadow", *e, *id)));
    for (pass, entity, id) in main.chain(prepass).chain(shadow) {
        if !made.contains(&id) && reported.insert(id) {
            warn!("precompile: {pass} pipeline {id:?} for {:?} wasn't precompiled ({})", entity.id(), why(entity));
        }
    }
}
