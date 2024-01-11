use std::marker::PhantomData;
use std::hash::Hash;
use bevy::core_pipeline::core_3d::{Transmissive3d, Transparent3d, Opaque3d, AlphaMask3d, ScreenSpaceTransmissionQuality};
use bevy::core_pipeline::deferred::{Opaque3dDeferred, AlphaMask3dDeferred};
use bevy::core_pipeline::prepass::{NormalPrepass, DepthPrepass, MotionVectorPrepass, DeferredPrepass, Opaque3dPrepass, AlphaMask3dPrepass};
use bevy::core_pipeline::tonemapping::{Tonemapping, DebandDither};
use bevy::ecs::system::ReadOnlySystemParam;
use bevy::prelude::*;
use bevy::render::camera::TemporalJitter;
use bevy::render::view::{VisibleEntities, ExtractedView};
use bevy::render::{RenderApp, Render, RenderSet};
use bevy::render::extract_instances::ExtractInstancesPlugin;
use bevy::render::render_asset::{RenderAssets, prepare_assets};
use bevy::render::render_phase::{DrawFunctions, AddRenderCommand, SetItemPipeline, RenderCommand, RenderPhase};
use bevy::render::render_resource::{SpecializedMeshPipelines, PipelineCache};
use bevy::pbr::*;

pub trait PrepassDrawCommand: Send + Sync + TypePath + Clone + Sized + 
RenderCommand<Shadow> + RenderCommand<Opaque3dPrepass> + 
RenderCommand<AlphaMask3dPrepass> + RenderCommand<Opaque3dDeferred> + RenderCommand<AlphaMask3dDeferred> {}
pub trait RenderDrawCommand: Send + Sync + TypePath + RenderCommand<Opaque3d> + RenderCommand<Transparent3d> + RenderCommand<Transmissive3d> + RenderCommand<AlphaMask3d> {}

type SpecializedDrawPrepass<M, F> = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetMaterialBindGroup<M, 1>,
    SetMeshBindGroup<2>,
    F,
);

type SpecializedDrawMaterial<M, F> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMaterialBindGroup<M, 1>,
    SetMeshBindGroup<2>,
    F,
);


/// * Superset of the MaterialPlugin from Bevy
/// * Lets you add a material to the app and override the final draw functions used during render and prepass steps
/// * DrawMesh is the default function used by bevy's material plugin for both render and prepass steps, start there to create your own version
pub struct SpecializedMaterialPlugin<M: Material, P: PrepassDrawCommand, R: RenderDrawCommand>{
    prepass_enabled: bool,
    _marker: PhantomData<M>,
    _marker2: PhantomData<P>,
    _marker3: PhantomData<R>,
}
impl<M: Material, P: PrepassDrawCommand, R: RenderDrawCommand> Default for SpecializedMaterialPlugin<M, P, R>{
    fn default() -> Self {
        Self{prepass_enabled: true, _marker: PhantomData::default(), _marker2: PhantomData::default(), _marker3: PhantomData::default()}
    }
}
impl<M: Material, P: PrepassDrawCommand, R: RenderDrawCommand> Plugin for SpecializedMaterialPlugin<M, P, R>
where
    M::Data: PartialEq + Eq + Hash + Clone,
    <P as RenderCommand<Shadow>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<Opaque3dPrepass>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<AlphaMask3dPrepass>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<Opaque3dDeferred>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<AlphaMask3dDeferred>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<Opaque3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<Transparent3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<Transmissive3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<AlphaMask3d>>::Param: ReadOnlySystemParam
{
    fn build(&self, app: &mut App) {
        app.init_asset::<M>()
            .add_plugins(ExtractInstancesPlugin::<AssetId<M>>::extract_visible());

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<DrawFunctions<Shadow>>()
                .add_render_command::<Shadow, SpecializedDrawPrepass<M, P>>()
                .add_render_command::<Transmissive3d, SpecializedDrawMaterial<M, R>>()
                .add_render_command::<Transparent3d, SpecializedDrawMaterial<M, R>>()
                .add_render_command::<Opaque3d, SpecializedDrawMaterial<M, R>>()
                .add_render_command::<AlphaMask3d, SpecializedDrawMaterial<M, R>>()
                .init_resource::<ExtractedMaterials<M>>()
                .init_resource::<RenderMaterials<M>>()
                .init_resource::<SpecializedMeshPipelines<MaterialPipeline<M>>>()
                .add_systems(ExtractSchedule, extract_materials::<M>)
                .add_systems(
                    Render,
                    (
                        prepare_materials::<M>
                            .in_set(RenderSet::PrepareAssets)
                            .after(prepare_assets::<Image>),
                        specialized_queue_shadows::<M, P>
                            .in_set(RenderSet::QueueMeshes)
                            .after(prepare_materials::<M>),
                        specialized_queue_material_meshes::<M, R>
                            .in_set(RenderSet::QueueMeshes)
                            .after(prepare_materials::<M>),
                    ),
                );
        }
        // PrepassPipelinePlugin is required for shadow mapping and the optional PrepassPlugin
        app.add_plugins(PrepassPipelinePlugin::<M>::default());

        if self.prepass_enabled {
            app.add_plugins(SpecializedPrepassPlugin::<M, P>::default());
        }
    }
    fn finish(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<MaterialPipeline<M>>();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn specialized_queue_shadows<M: Material, P: PrepassDrawCommand>(
    shadow_draw_functions: Res<DrawFunctions<Shadow>>,
    prepass_pipeline: Res<PrepassPipeline<M>>,
    render_meshes: Res<RenderAssets<Mesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_materials: Res<RenderMaterials<M>>,
    render_material_instances: Res<RenderMaterialInstances<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<PrepassPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    view_lights: Query<(Entity, &ViewLightEntities)>,
    mut view_light_shadow_phases: Query<(&LightEntity, &mut RenderPhase<Shadow>)>,
    point_light_entities: Query<&CubemapVisibleEntities, With<ExtractedPointLight>>,
    directional_light_entities: Query<&CascadesVisibleEntities, With<ExtractedDirectionalLight>>,
    spot_light_entities: Query<&VisibleEntities, With<ExtractedPointLight>>,
) where
    M::Data: PartialEq + Eq + Hash + Clone
{
    for (entity, view_lights) in &view_lights {
        let draw_shadow_mesh = shadow_draw_functions.read().id::<SpecializedDrawPrepass<M, P>>();
        for view_light_entity in view_lights.lights.iter().copied() {
            let (light_entity, mut shadow_phase) =
                view_light_shadow_phases.get_mut(view_light_entity).unwrap();
            let is_directional_light = matches!(light_entity, LightEntity::Directional { .. });
            let visible_entities = match light_entity {
                LightEntity::Directional {
                    light_entity,
                    cascade_index,
                } => directional_light_entities
                    .get(*light_entity)
                    .expect("Failed to get directional light visible entities")
                    .entities
                    .get(&entity)
                    .expect("Failed to get directional light visible entities for view")
                    .get(*cascade_index)
                    .expect("Failed to get directional light visible entities for cascade"),
                LightEntity::Point {
                    light_entity,
                    face_index,
                } => point_light_entities
                    .get(*light_entity)
                    .expect("Failed to get point light visible entities")
                    .get(*face_index),
                LightEntity::Spot { light_entity } => spot_light_entities
                    .get(*light_entity)
                    .expect("Failed to get spot light visible entities"),
            };
            // NOTE: Lights with shadow mapping disabled will have no visible entities
            // so no meshes will be queued
            for entity in visible_entities.iter().copied() {
                let Some(mesh_instance) = render_mesh_instances.get(&entity) else {
                    continue;
                };
                if !mesh_instance.shadow_caster {
                    continue;
                }
                let Some(material_asset_id) = render_material_instances.get(&entity) else {
                    continue;
                };
                let Some(material) = render_materials.get(material_asset_id) else {
                    continue;
                };
                let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                    continue;
                };

                let mut mesh_key =
                    MeshPipelineKey::from_primitive_topology(mesh.primitive_topology)
                        | MeshPipelineKey::DEPTH_PREPASS;
                if mesh.morph_targets.is_some() {
                    mesh_key |= MeshPipelineKey::MORPH_TARGETS;
                }
                if is_directional_light {
                    mesh_key |= MeshPipelineKey::DEPTH_CLAMP_ORTHO;
                }
                mesh_key |= match material.properties.alpha_mode {
                    AlphaMode::Mask(_)
                    | AlphaMode::Blend
                    | AlphaMode::Premultiplied
                    | AlphaMode::Add => MeshPipelineKey::MAY_DISCARD,
                    _ => MeshPipelineKey::NONE,
                };
                let pipeline_id = pipelines.specialize(
                    &pipeline_cache,
                    &prepass_pipeline,
                    MaterialPipelineKey {
                        mesh_key,
                        bind_group_data: material.key.clone(),
                    },
                    &mesh.layout,
                );

                let pipeline_id = match pipeline_id {
                    Ok(id) => id,
                    Err(err) => {
                        error!("{}", err);
                        continue;
                    }
                };

                shadow_phase.add(Shadow {
                    draw_function: draw_shadow_mesh,
                    pipeline: pipeline_id,
                    entity,
                    distance: 0.0, // TODO: sort front-to-back
                    batch_range: 0..1,
                    dynamic_offset: None,
                });
            }
        }
    }
}

const fn alpha_mode_pipeline_key(alpha_mode: AlphaMode) -> MeshPipelineKey {
    match alpha_mode {
        // Premultiplied and Add share the same pipeline key
        // They're made distinct in the PBR shader, via `premultiply_alpha()`
        AlphaMode::Premultiplied | AlphaMode::Add => MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA,
        AlphaMode::Blend => MeshPipelineKey::BLEND_ALPHA,
        AlphaMode::Multiply => MeshPipelineKey::BLEND_MULTIPLY,
        AlphaMode::Mask(_) => MeshPipelineKey::MAY_DISCARD,
        _ => MeshPipelineKey::NONE,
    }
}

const fn tonemapping_pipeline_key(tonemapping: Tonemapping) -> MeshPipelineKey {
    match tonemapping {
        Tonemapping::None => MeshPipelineKey::TONEMAP_METHOD_NONE,
        Tonemapping::Reinhard => MeshPipelineKey::TONEMAP_METHOD_REINHARD,
        Tonemapping::ReinhardLuminance => MeshPipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE,
        Tonemapping::AcesFitted => MeshPipelineKey::TONEMAP_METHOD_ACES_FITTED,
        Tonemapping::AgX => MeshPipelineKey::TONEMAP_METHOD_AGX,
        Tonemapping::SomewhatBoringDisplayTransform => {
            MeshPipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
        }
        Tonemapping::TonyMcMapface => MeshPipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE,
        Tonemapping::BlenderFilmic => MeshPipelineKey::TONEMAP_METHOD_BLENDER_FILMIC,
    }
}

const fn screen_space_specular_transmission_pipeline_key(
    screen_space_transmissive_blur_quality: ScreenSpaceTransmissionQuality,
) -> MeshPipelineKey {
    match screen_space_transmissive_blur_quality {
        ScreenSpaceTransmissionQuality::Low => {
            MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_LOW
        }
        ScreenSpaceTransmissionQuality::Medium => {
            MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_MEDIUM
        }
        ScreenSpaceTransmissionQuality::High => {
            MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_HIGH
        }
        ScreenSpaceTransmissionQuality::Ultra => {
            MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_ULTRA
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn specialized_queue_material_meshes<M: Material, R: RenderDrawCommand>(
    opaque_draw_functions: Res<DrawFunctions<Opaque3d>>,
    alpha_mask_draw_functions: Res<DrawFunctions<AlphaMask3d>>,
    transmissive_draw_functions: Res<DrawFunctions<Transmissive3d>>,
    transparent_draw_functions: Res<DrawFunctions<Transparent3d>>,
    material_pipeline: Res<MaterialPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<MaterialPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    msaa: Res<Msaa>,
    render_meshes: Res<RenderAssets<Mesh>>,
    render_materials: Res<RenderMaterials<M>>,
    mut render_mesh_instances: ResMut<RenderMeshInstances>,
    render_material_instances: Res<RenderMaterialInstances<M>>,
    images: Res<RenderAssets<Image>>,
    mut views: Query<(
        &ExtractedView,
        &VisibleEntities,
        Option<&Tonemapping>,
        Option<&DebandDither>,
        Option<&EnvironmentMapLight>,
        Option<&ShadowFilteringMethod>,
        Option<&ScreenSpaceAmbientOcclusionSettings>,
        (
            Has<NormalPrepass>,
            Has<DepthPrepass>,
            Has<MotionVectorPrepass>,
            Has<DeferredPrepass>,
        ),
        Option<&Camera3d>,
        Option<&TemporalJitter>,
        Option<&Projection>,
        &mut RenderPhase<Opaque3d>,
        &mut RenderPhase<AlphaMask3d>,
        &mut RenderPhase<Transmissive3d>,
        &mut RenderPhase<Transparent3d>,
    )>,
) where
    M::Data: PartialEq + Eq + Hash + Clone,
    <R as RenderCommand<Opaque3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<Transparent3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<Transmissive3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand<AlphaMask3d>>::Param: ReadOnlySystemParam
{
    for (
        view,
        visible_entities,
        tonemapping,
        dither,
        environment_map,
        shadow_filter_method,
        ssao,
        (normal_prepass, depth_prepass, motion_vector_prepass, deferred_prepass),
        camera_3d,
        temporal_jitter,
        projection,
        mut opaque_phase,
        mut alpha_mask_phase,
        mut transmissive_phase,
        mut transparent_phase,
    ) in &mut views
    {
        let draw_opaque_pbr = opaque_draw_functions.read().id::<SpecializedDrawMaterial<M, R>>();
        let draw_alpha_mask_pbr = alpha_mask_draw_functions.read().id::<SpecializedDrawMaterial<M, R>>();
        let draw_transmissive_pbr = transmissive_draw_functions.read().id::<SpecializedDrawMaterial<M, R>>();
        let draw_transparent_pbr = transparent_draw_functions.read().id::<SpecializedDrawMaterial<M, R>>();

        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        if normal_prepass {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }

        if depth_prepass {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }

        if motion_vector_prepass {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }

        if deferred_prepass {
            view_key |= MeshPipelineKey::DEFERRED_PREPASS;
        }

        if temporal_jitter.is_some() {
            view_key |= MeshPipelineKey::TEMPORAL_JITTER;
        }

        let environment_map_loaded = environment_map.is_some_and(|map| map.is_loaded(&images));

        if environment_map_loaded {
            view_key |= MeshPipelineKey::ENVIRONMENT_MAP;
        }

        if let Some(projection) = projection {
            view_key |= match projection {
                Projection::Perspective(_) => MeshPipelineKey::VIEW_PROJECTION_PERSPECTIVE,
                Projection::Orthographic(_) => MeshPipelineKey::VIEW_PROJECTION_ORTHOGRAPHIC,
            };
        }

        match shadow_filter_method.unwrap_or(&ShadowFilteringMethod::default()) {
            ShadowFilteringMethod::Hardware2x2 => {
                view_key |= MeshPipelineKey::SHADOW_FILTER_METHOD_HARDWARE_2X2;
            }
            ShadowFilteringMethod::Castano13 => {
                view_key |= MeshPipelineKey::SHADOW_FILTER_METHOD_CASTANO_13;
            }
            ShadowFilteringMethod::Jimenez14 => {
                view_key |= MeshPipelineKey::SHADOW_FILTER_METHOD_JIMENEZ_14;
            }
        }

        if !view.hdr {
            if let Some(tonemapping) = tonemapping {
                view_key |= MeshPipelineKey::TONEMAP_IN_SHADER;
                view_key |= tonemapping_pipeline_key(*tonemapping);
            }
            if let Some(DebandDither::Enabled) = dither {
                view_key |= MeshPipelineKey::DEBAND_DITHER;
            }
        }
        if ssao.is_some() {
            view_key |= MeshPipelineKey::SCREEN_SPACE_AMBIENT_OCCLUSION;
        }
        if let Some(camera_3d) = camera_3d {
            view_key |= screen_space_specular_transmission_pipeline_key(
                camera_3d.screen_space_specular_transmission_quality,
            );
        }
        let rangefinder = view.rangefinder3d();
        for visible_entity in &visible_entities.entities {
            let Some(material_asset_id) = render_material_instances.get(visible_entity) else {
                continue;
            };
            let Some(mesh_instance) = render_mesh_instances.get_mut(visible_entity) else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let Some(material) = render_materials.get(material_asset_id) else {
                continue;
            };

            let forward = match material.properties.render_method {
                OpaqueRendererMethod::Forward => true,
                OpaqueRendererMethod::Deferred => false,
                OpaqueRendererMethod::Auto => unreachable!(),
            };

            let mut mesh_key = view_key;

            mesh_key |= MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);

            if mesh.morph_targets.is_some() {
                mesh_key |= MeshPipelineKey::MORPH_TARGETS;
            }
            mesh_key |= alpha_mode_pipeline_key(material.properties.alpha_mode);

            let pipeline_id = pipelines.specialize(
                &pipeline_cache,
                &material_pipeline,
                MaterialPipelineKey {
                    mesh_key,
                    bind_group_data: material.key.clone(),
                },
                &mesh.layout,
            );
            let pipeline_id = match pipeline_id {
                Ok(id) => id,
                Err(err) => {
                    error!("{}", err);
                    continue;
                }
            };

            mesh_instance.material_bind_group_id = material.get_bind_group_id();

            let distance = rangefinder
                .distance_translation(&mesh_instance.transforms.transform.translation)
                + material.properties.depth_bias;
            match material.properties.alpha_mode {
                AlphaMode::Opaque => {
                    if material.properties.reads_view_transmission_texture {
                        transmissive_phase.add(Transmissive3d {
                            entity: *visible_entity,
                            draw_function: draw_transmissive_pbr,
                            pipeline: pipeline_id,
                            distance,
                            batch_range: 0..1,
                            dynamic_offset: None,
                        });
                    } else if forward {
                        opaque_phase.add(Opaque3d {
                            entity: *visible_entity,
                            draw_function: draw_opaque_pbr,
                            pipeline: pipeline_id,
                            distance,
                            batch_range: 0..1,
                            dynamic_offset: None,
                        });
                    }
                }
                AlphaMode::Mask(_) => {
                    if material.properties.reads_view_transmission_texture {
                        transmissive_phase.add(Transmissive3d {
                            entity: *visible_entity,
                            draw_function: draw_transmissive_pbr,
                            pipeline: pipeline_id,
                            distance,
                            batch_range: 0..1,
                            dynamic_offset: None,
                        });
                    } else if forward {
                        alpha_mask_phase.add(AlphaMask3d {
                            entity: *visible_entity,
                            draw_function: draw_alpha_mask_pbr,
                            pipeline: pipeline_id,
                            distance,
                            batch_range: 0..1,
                            dynamic_offset: None,
                        });
                    }
                }
                AlphaMode::Blend
                | AlphaMode::Premultiplied
                | AlphaMode::Add
                | AlphaMode::Multiply => {
                    transparent_phase.add(Transparent3d {
                        entity: *visible_entity,
                        draw_function: draw_transparent_pbr,
                        pipeline: pipeline_id,
                        distance,
                        batch_range: 0..1,
                        dynamic_offset: None,
                    });
                }
            }
        }
    }
}

/// Sets up the prepasses for a [`Material`].
///
/// This depends on the [`PrepassPipelinePlugin`].
struct SpecializedPrepassPlugin<M: Material, P: PrepassDrawCommand>(PhantomData<(M, P)>);

impl<M: Material, P: PrepassDrawCommand> Default for SpecializedPrepassPlugin<M, P> {
    fn default() -> Self {
        Self{0: PhantomData::default()}
    }
}

impl<M: Material, P: PrepassDrawCommand> Plugin for SpecializedPrepassPlugin<M, P>
where
    M::Data: PartialEq + Eq + Hash + Clone,
    <P as RenderCommand<Opaque3dPrepass>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<AlphaMask3dPrepass>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<Opaque3dDeferred>>::Param: ReadOnlySystemParam,
    <P as RenderCommand<AlphaMask3dDeferred>>::Param: ReadOnlySystemParam
{
    fn build(&self, app: &mut App) {
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_render_command::<Opaque3dPrepass, SpecializedDrawPrepass<M, P>>()
            .add_render_command::<AlphaMask3dPrepass, SpecializedDrawPrepass<M, P>>()
            .add_render_command::<Opaque3dDeferred, SpecializedDrawPrepass<M, P>>()
            .add_render_command::<AlphaMask3dDeferred, SpecializedDrawPrepass<M, P>>()
            .add_systems(
                Render,
                specialized_queue_prepass_material_meshes::<M, P>
                    .in_set(RenderSet::QueueMeshes)
                    .after(prepare_materials::<M>),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn specialized_queue_prepass_material_meshes<M: Material, P: PrepassDrawCommand>(
    opaque_draw_functions: Res<DrawFunctions<Opaque3dPrepass>>,
    alpha_mask_draw_functions: Res<DrawFunctions<AlphaMask3dPrepass>>,
    opaque_deferred_draw_functions: Res<DrawFunctions<Opaque3dDeferred>>,
    alpha_mask_deferred_draw_functions: Res<DrawFunctions<AlphaMask3dDeferred>>,
    prepass_pipeline: Res<PrepassPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<PrepassPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    msaa: Res<Msaa>,
    render_meshes: Res<RenderAssets<Mesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_materials: Res<RenderMaterials<M>>,
    render_material_instances: Res<RenderMaterialInstances<M>>,
    mut views: Query<
        (
            &ExtractedView,
            &VisibleEntities,
            Option<&mut RenderPhase<Opaque3dPrepass>>,
            Option<&mut RenderPhase<AlphaMask3dPrepass>>,
            Option<&mut RenderPhase<Opaque3dDeferred>>,
            Option<&mut RenderPhase<AlphaMask3dDeferred>>,
            Option<&DepthPrepass>,
            Option<&NormalPrepass>,
            Option<&MotionVectorPrepass>,
            Option<&DeferredPrepass>,
        ),
        Or<(
            With<RenderPhase<Opaque3dPrepass>>,
            With<RenderPhase<AlphaMask3dPrepass>>,
            With<RenderPhase<Opaque3dDeferred>>,
            With<RenderPhase<AlphaMask3dDeferred>>,
        )>,
    >,
) where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    let opaque_draw_prepass = opaque_draw_functions
        .read()
        .get_id::<SpecializedDrawPrepass<M, P>>()
        .unwrap();
    let alpha_mask_draw_prepass = alpha_mask_draw_functions
        .read()
        .get_id::<SpecializedDrawPrepass<M, P>>()
        .unwrap();
    let opaque_draw_deferred = opaque_deferred_draw_functions
        .read()
        .get_id::<SpecializedDrawPrepass<M, P>>()
        .unwrap();
    let alpha_mask_draw_deferred = alpha_mask_deferred_draw_functions
        .read()
        .get_id::<SpecializedDrawPrepass<M, P>>()
        .unwrap();
    for (
        view,
        visible_entities,
        mut opaque_phase,
        mut alpha_mask_phase,
        mut opaque_deferred_phase,
        mut alpha_mask_deferred_phase,
        depth_prepass,
        normal_prepass,
        motion_vector_prepass,
        deferred_prepass,
    ) in &mut views
    {
        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples());
        if depth_prepass.is_some() {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }
        if normal_prepass.is_some() {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }
        if motion_vector_prepass.is_some() {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }

        let rangefinder = view.rangefinder3d();

        for visible_entity in &visible_entities.entities {
            let Some(material_asset_id) = render_material_instances.get(visible_entity) else {
                continue;
            };
            let Some(mesh_instance) = render_mesh_instances.get(visible_entity) else {
                continue;
            };
            let Some(material) = render_materials.get(material_asset_id) else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };

            let mut mesh_key =
                MeshPipelineKey::from_primitive_topology(mesh.primitive_topology) | view_key;
            if mesh.morph_targets.is_some() {
                mesh_key |= MeshPipelineKey::MORPH_TARGETS;
            }
            let alpha_mode = material.properties.alpha_mode;
            match alpha_mode {
                AlphaMode::Opaque => {}
                AlphaMode::Mask(_) => mesh_key |= MeshPipelineKey::MAY_DISCARD,
                AlphaMode::Blend
                | AlphaMode::Premultiplied
                | AlphaMode::Add
                | AlphaMode::Multiply => continue,
            }

            if material.properties.reads_view_transmission_texture {
                // No-op: Materials reading from `ViewTransmissionTexture` are not rendered in the `Opaque3d`
                // phase, and are therefore also excluded from the prepass much like alpha-blended materials.
                continue;
            }

            let forward = match material.properties.render_method {
                OpaqueRendererMethod::Forward => true,
                OpaqueRendererMethod::Deferred => false,
                OpaqueRendererMethod::Auto => unreachable!(),
            };

            let deferred = deferred_prepass.is_some() && !forward;

            if deferred {
                mesh_key |= MeshPipelineKey::DEFERRED_PREPASS;
            }

            let pipeline_id = pipelines.specialize(
                &pipeline_cache,
                &prepass_pipeline,
                MaterialPipelineKey {
                    mesh_key,
                    bind_group_data: material.key.clone(),
                },
                &mesh.layout,
            );
            let pipeline_id = match pipeline_id {
                Ok(id) => id,
                Err(err) => {
                    error!("{}", err);
                    continue;
                }
            };

            let distance = rangefinder
                .distance_translation(&mesh_instance.transforms.transform.translation)
                + material.properties.depth_bias;
            match alpha_mode {
                AlphaMode::Opaque => {
                    if deferred {
                        opaque_deferred_phase
                            .as_mut()
                            .unwrap()
                            .add(Opaque3dDeferred {
                                entity: *visible_entity,
                                draw_function: opaque_draw_deferred,
                                pipeline_id,
                                distance,
                                batch_range: 0..1,
                                dynamic_offset: None,
                            });
                    } else if let Some(opaque_phase) = opaque_phase.as_mut() {
                        opaque_phase.add(Opaque3dPrepass {
                            entity: *visible_entity,
                            draw_function: opaque_draw_prepass,
                            pipeline_id,
                            distance,
                            batch_range: 0..1,
                            dynamic_offset: None,
                        });
                    }
                }
                AlphaMode::Mask(_) => {
                    if deferred {
                        alpha_mask_deferred_phase
                            .as_mut()
                            .unwrap()
                            .add(AlphaMask3dDeferred {
                                entity: *visible_entity,
                                draw_function: alpha_mask_draw_deferred,
                                pipeline_id,
                                distance,
                                batch_range: 0..1,
                                dynamic_offset: None,
                            });
                    } else if let Some(alpha_mask_phase) = alpha_mask_phase.as_mut() {
                        alpha_mask_phase.add(AlphaMask3dPrepass {
                            entity: *visible_entity,
                            draw_function: alpha_mask_draw_prepass,
                            pipeline_id,
                            distance,
                            batch_range: 0..1,
                            dynamic_offset: None,
                        });
                    }
                }
                AlphaMode::Blend
                | AlphaMode::Premultiplied
                | AlphaMode::Add
                | AlphaMode::Multiply => {}
            }
        }
    }
}
