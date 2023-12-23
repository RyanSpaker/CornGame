use std::marker::PhantomData;
use std::hash::Hash;
use bevy::{
    pbr::*,
    render::{
        mesh::{MeshVertexBufferLayout, MeshVertexAttribute, GpuBufferInfo, GpuMesh},
        render_resource::*, 
        render_phase::{SetItemPipeline, RenderCommand, PhaseItem, TrackedRenderPass, 
            RenderCommandResult, AddRenderCommand, DrawFunctions, RenderPhase}, 
        render_asset::RenderAssets, RenderApp, view::ExtractedView, RenderSet, Render, Extract
    }, 
    asset::Handle, 
    prelude::*, 
    ecs::system::{lifetimeless::{SRes, Read}, SystemParamItem}, 
    core_pipeline::{
        core_3d::Opaque3d, 
        experimental::taa::TemporalAntiAliasSettings, 
        prepass::NormalPrepass, 
        tonemapping::{DebandDither, Tonemapping}
    }, math::Mat3A
};
use crate::prelude::corn_model::CornMeshes;
use super::CornInstanceBuffer;

bitflags::bitflags! {
    #[repr(transparent)]
    struct MeshFlags: u32 {
        const SHADOW_RECEIVER            = (1 << 0);
        // Indicates the sign of the determinant of the 3x3 model matrix. If the sign is positive,
        // then the flag should be set, else it should not be set.
        const SIGN_DETERMINANT_MODEL_3X3 = (1 << 31);
        const NONE                       = 0;
        const UNINITIALIZED              = 0xFFFF;
    }
}

#[derive(Resource)]
pub struct CornMaterialPipeline<M> where M: Material{
    pub material_pipeline: MaterialPipeline<M>,
    pub vertex_shader: Option<Handle<Shader>>,
    pub fragment_shader: Option<Handle<Shader>>,
    marker: PhantomData<M>,
}
impl<M: Material> Clone for CornMaterialPipeline<M> {
    fn clone(&self) -> Self {
        Self {
            material_pipeline: self.material_pipeline.clone(),
            vertex_shader: self.vertex_shader.clone(),
            fragment_shader: self.fragment_shader.clone(),
            marker: PhantomData,
        }
    }
}
impl<M: Material> SpecializedMeshPipeline for CornMaterialPipeline<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    type Key = (MaterialPipelineKey<M>, CornPipelineKey);

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.material_pipeline.specialize(key.0, layout)?;
        if let Some(vertex_shader) = &self.vertex_shader {
            descriptor.vertex.shader = vertex_shader.clone();
        }
        if let Some(fragment_shader) = &self.fragment_shader {
            descriptor.fragment.as_mut().unwrap().shader = fragment_shader.clone();
        }
        descriptor.vertex.shader_defs.push(ShaderDefVal::Bool("CORN_INSTANCED".to_string(), true));
        
        let extra_attr = layout.get_layout(&[
            MeshVertexAttribute::new("Mesh_Index", 7, VertexFormat::Uint32).at_shader_location(7)
        ])?;
        let Some(buffer_layout) = descriptor.vertex.buffers.get_mut(0) else{
            panic!("material_pipeline.specialize didnt assign any vertex buffer layouts");
        };
        buffer_layout.attributes.push(extra_attr.attributes[0]);
        descriptor.vertex.buffers.push(VertexBufferLayout { 
            array_stride: 32, 
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute{shader_location: 8, format: VertexFormat::Float32x4, offset: 0},
                VertexAttribute{shader_location: 9, format: VertexFormat::Float32x2, offset: 16},
                VertexAttribute{shader_location: 10, format: VertexFormat::Uint32x2, offset: 24},
            ] 
        });
        return Ok(descriptor);
    }
}
impl<M: Material> FromWorld for CornMaterialPipeline<M>{
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        let material_pipeline = MaterialPipeline::<M>::from_world(world);
        let asset_server = world.resource::<AssetServer>();
        let vertex_shader = Some(asset_server.load("shaders/corn/pbr_vertex.wgsl"));
        //let fragment_shader = Some(asset_server.load("shaders/corn/pbr_vertex.wgsl"));//material_pipeline.fragment_shader.clone();
        CornMaterialPipeline{
            material_pipeline,
            vertex_shader,
            fragment_shader: None,
            marker: PhantomData,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct CornPipelineKey{

}

#[derive(Component)]
pub struct InstancedDrawingCommand<T> where T: Send + Sync{
    pub render_data: T,
    pub draw_command: for<'w> fn(&mut TrackedRenderPass<'w>, &'w RenderAssets<Mesh>, &'w T) -> RenderCommandResult,
}
pub struct CornRenderData{
    instance_buffer: CornInstanceBuffer,
    global_mesh: Handle<Mesh>,
}
pub fn draw_corn_instanced_opaque<'w>(
    render_pass: &mut TrackedRenderPass<'w>, 
    meshes: &'w RenderAssets<Mesh>, 
    corn_data: &'w CornRenderData
) -> RenderCommandResult{
    if !corn_data.instance_buffer.ready_to_render() {return RenderCommandResult::Failure;}
    let Some(global_mesh) = meshes.get(&corn_data.global_mesh) else{
        return RenderCommandResult::Failure;
    };
    let Some(data_buffer) = corn_data.instance_buffer.sorted_buffer.as_ref() else{
        return RenderCommandResult::Failure;
    };
    let Some(indirect_buffer) = corn_data.instance_buffer.indirect_buffer.as_ref() else {
        return RenderCommandResult::Failure;
    };
    match &global_mesh.buffer_info{
        GpuBufferInfo::Indexed { buffer, index_format, .. } =>{
            render_pass.set_vertex_buffer(0, global_mesh.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, data_buffer.slice(..));
            render_pass.set_index_buffer(buffer.slice(..), 0, *index_format);
            render_pass.multi_draw_indexed_indirect(
                &indirect_buffer, 
                0, 
                corn_data.instance_buffer.lod_count.clone()
            );
            return RenderCommandResult::Success;
        }
        GpuBufferInfo::NonIndexed => {return RenderCommandResult::Failure;}
    };
}


type DrawMaterialInstanced<M, T> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMaterialBindGroup<M, 1>,
    SetMeshBindGroup<2>,
    DrawMeshInstanced<T>,
);

//Draw Function for instanced corn
pub struct DrawMeshInstanced<T>(PhantomData<T>);
impl<P: PhaseItem, T: 'static + Send + Sync> RenderCommand<P> for DrawMeshInstanced<T> {
    type Param = SRes<RenderAssets<Mesh>>;
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<InstancedDrawingCommand<T>>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        command: &'w InstancedDrawingCommand<T>,
        meshes: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        (command.draw_command)(pass, meshes.into_inner(), &command.render_data)
    }
}

pub struct DebugDraw<const M: usize>;
impl<P: PhaseItem, const M: usize> RenderCommand<P> for DebugDraw<M>{
    type Param = ();
    type ViewWorldQuery = ();
    type ItemWorldQuery = ();

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        _entity: (),
        _world: (),
        _pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        println!("{}", M);
        RenderCommandResult::Success
    }
}

fn spawn_field_entity(
    main_corn: Extract<Res<CornMeshes>>,
    instance_buffer: ResMut<CornInstanceBuffer>,
    mut commands: Commands
){
    if !instance_buffer.ready_to_render() || main_corn.global_mesh.is_none(){return;}
    let cur_data: CornRenderData = CornRenderData { 
        instance_buffer: instance_buffer.clone(), 
        global_mesh: main_corn.global_mesh.as_ref().unwrap().clone()};
    let mut flags = MeshFlags::SHADOW_RECEIVER;
    let transform = GlobalTransform::IDENTITY.compute_matrix();
    if Mat3A::from_mat4(transform).determinant().is_sign_positive() {
        flags |= MeshFlags::SIGN_DETERMINANT_MODEL_3X3;
    }
    commands.spawn((
        InstancedDrawingCommand::<CornRenderData>{render_data: cur_data, draw_command: draw_corn_instanced_opaque},
        main_corn.global_mesh.as_ref().unwrap().to_owned(),
        MeshUniform {
            flags: flags.bits(),
            transform,
            previous_transform: transform,
            inverse_transpose_model: transform.inverse().transpose(),
        },
        main_corn.materials.get(&"CornLeaves".to_string()).unwrap().to_owned()
    ));
}

#[allow(clippy::too_many_arguments)]
fn corn_field_queue(
    instance_buffer: Res<CornInstanceBuffer>,
    corn_mesh: Res<CornMeshes>,
    opaque_draw_functions: Res<DrawFunctions<Opaque3d>>,
    corn_fields: Query<Entity, With<InstancedDrawingCommand<CornRenderData>>>,
    mut views: Query<(
        &ExtractedView,
        Option<&Tonemapping>,
        Option<&DebandDither>,
        Option<&EnvironmentMapLight>,
        Option<&ScreenSpaceAmbientOcclusionSettings>,
        Option<&NormalPrepass>,
        Option<&TemporalAntiAliasSettings>,
        &mut RenderPhase<Opaque3d>,
    )>,
    material_pipeline: Res<CornMaterialPipeline<StandardMaterial>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CornMaterialPipeline<StandardMaterial>>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<Mesh>>,
    materials: Res<RenderMaterials<StandardMaterial>>,
    msaa: Res<Msaa>,
    images: Res<RenderAssets<Image>>,
)
{
    //return if we are not ready to render corn
    if !instance_buffer.ready_to_render(){return;}
    if corn_fields.is_empty() {return;}
    //get our draw functions
    let opaque_draw = opaque_draw_functions.read().id::<DrawMaterialInstanced<StandardMaterial, CornRenderData>>();
    //get our mesh and material
    let (Some(mesh), Some(material)) = (
        meshes.get(corn_mesh.global_mesh.as_ref().unwrap()),
        materials.get(corn_mesh.materials.get(&"CornLeaves".to_string()).unwrap())
    ) else{
        return;
    };
    //Spawn an entity to hold rendering data:
    let entity = corn_fields.single();
    
    for (
        view,
        tonemapping,
        dither,
        environment_map,
        ssao,
        normal_prepass,
        taa_settings,
        mut opaque_phase,
    ) in &mut views
    {
        let material_key = get_material_key(
            &*msaa, 
            view, 
            &normal_prepass, 
            &taa_settings, 
            &environment_map, 
            &*images, 
            &tonemapping, 
            &dither, 
            &ssao, 
            mesh, 
            material
        );
        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &material_pipeline,
            (material_key, CornPipelineKey{}),
            &mesh.layout,
        ).unwrap();
        opaque_phase.add(Opaque3d {
            entity,
            draw_function: opaque_draw,
            pipeline: pipeline_id,
            distance: 0.0,
        });
    }
}

pub fn get_material_key<M: Material>(
    msaa: &Msaa,
    view: &ExtractedView,
    normal_prepass: &Option<&NormalPrepass>,
    taa_settings: &Option<&TemporalAntiAliasSettings>,
    environment_map: &Option<&EnvironmentMapLight>,
    images: &RenderAssets<Image>,
    tonemapping: &Option<&Tonemapping>,
    dither: &Option<&DebandDither>,
    ssao: &Option<&ScreenSpaceAmbientOcclusionSettings>,
    mesh: &GpuMesh,
    material: &PreparedMaterial<M>
) -> MaterialPipelineKey<M> where M::Data: Clone
{
    //View Key:
    let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
        | MeshPipelineKey::from_hdr(view.hdr);
    if normal_prepass.is_some() {
        view_key |= MeshPipelineKey::NORMAL_PREPASS;
    }
    if taa_settings.is_some() {
        view_key |= MeshPipelineKey::TAA;
    }
    let environment_map_loaded = match environment_map {
        Some(environment_map) => environment_map.is_loaded(&images),
        None => false,
    };
    if environment_map_loaded {
        view_key |= MeshPipelineKey::ENVIRONMENT_MAP;
    }
    if !view.hdr {
        if let Some(tonemapping) = tonemapping {
            view_key |= MeshPipelineKey::TONEMAP_IN_SHADER;
            view_key |= match tonemapping {
                Tonemapping::None => MeshPipelineKey::TONEMAP_METHOD_NONE,
                Tonemapping::Reinhard => MeshPipelineKey::TONEMAP_METHOD_REINHARD,
                Tonemapping::ReinhardLuminance => {
                    MeshPipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE
                }
                Tonemapping::AcesFitted => MeshPipelineKey::TONEMAP_METHOD_ACES_FITTED,
                Tonemapping::AgX => MeshPipelineKey::TONEMAP_METHOD_AGX,
                Tonemapping::SomewhatBoringDisplayTransform => {
                    MeshPipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
                }
                Tonemapping::TonyMcMapface => MeshPipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE,
                Tonemapping::BlenderFilmic => MeshPipelineKey::TONEMAP_METHOD_BLENDER_FILMIC,
            };
        }
        if let Some(DebandDither::Enabled) = dither {
            view_key |= MeshPipelineKey::DEBAND_DITHER;
        }
    }
    if ssao.is_some() {
        view_key |= MeshPipelineKey::SCREEN_SPACE_AMBIENT_OCCLUSION;
    }
    //Mesh Key:
    let mut mesh_key = 
        MeshPipelineKey::from_primitive_topology(mesh.primitive_topology) | view_key;
    if mesh.morph_targets.is_some() {
        mesh_key |= MeshPipelineKey::MORPH_TARGETS;
    }
    match material.properties.alpha_mode {
        AlphaMode::Blend => {
            mesh_key |= MeshPipelineKey::BLEND_ALPHA;
        }
        AlphaMode::Premultiplied | AlphaMode::Add => {
            // Premultiplied and Add share the same pipeline key
            // They're made distinct in the PBR shader, via `premultiply_alpha()`
            mesh_key |= MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA;
        }
        AlphaMode::Multiply => {
            mesh_key |= MeshPipelineKey::BLEND_MULTIPLY;
        }
        AlphaMode::Mask(_) => {
            mesh_key |= MeshPipelineKey::MAY_DISCARD;
        }
        _ => (),
    }
    //Material Key
    MaterialPipelineKey {
        mesh_key,
        bind_group_data: material.key.clone(),
    }
}

pub struct MasterCornRenderPlugin;
impl Plugin for MasterCornRenderPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.sub_app_mut(RenderApp)
        .add_render_command::<Opaque3d, DrawMaterialInstanced<StandardMaterial, CornRenderData>>()
        .init_resource::<SpecializedMeshPipelines<CornMaterialPipeline<StandardMaterial>>>()
        .add_systems(Render, 
            corn_field_queue.in_set(RenderSet::Queue)
        )
        .add_systems(ExtractSchedule, spawn_field_entity);
    }
    fn finish(&self, app: &mut bevy::prelude::App) {
        app.sub_app_mut(RenderApp).init_resource::<CornMaterialPipeline<StandardMaterial>>();
    }
}
