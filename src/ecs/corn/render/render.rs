use bevy::{
    prelude::*,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialMeshBundle, RenderMeshInstances, StandardMaterial}, 
    render::{batching::NoAutomaticBatching, mesh::{Mesh, GpuBufferInfo}, render_asset::RenderAssets, render_phase::{PhaseItem, RenderCommand, TrackedRenderPass, RenderCommandResult}, render_resource::{AsBindGroup, ShaderDefVal, VertexBufferLayout}, view::NoFrustumCulling}, 
    asset::{Asset, Assets}, 
    reflect::Reflect, ecs::system::{lifetimeless::SRes, Commands, Res, ResMut, SystemParamItem}
};
use wgpu::{vertex_attr_array, ShaderStages, PushConstantRange};
use crate::{ecs::corn::buffer::{CornInstanceBuffer, CORN_DATA_SIZE}, prelude::corn_model::CornMeshes, util::specialized_material::{SpecializedDrawMaterial, SpecializedDrawPrepass, SpecializedMaterialPlugin}};

pub type CornMaterial = ExtendedMaterial<StandardMaterial, CornMaterialExtension>;

pub type CornDrawRender = SpecializedDrawMaterial<CornMaterial, DrawCorn>;

pub type CornDrawPrepass = SpecializedDrawPrepass<CornMaterial, DrawCorn>;

#[derive(Default, Debug, Clone, AsBindGroup, Asset, Reflect)]
pub struct CornMaterialExtension{}
impl MaterialExtension for CornMaterialExtension{
    fn vertex_shader() -> bevy::render::render_resource::ShaderRef {
        "shaders/corn/instanced_vertex.wgsl".into()
    }
    fn prepass_vertex_shader() -> bevy::render::render_resource::ShaderRef {
        "shaders/corn/prepass_instanced_vertex.wgsl".into()
    }
    fn deferred_vertex_shader() -> bevy::render::render_resource::ShaderRef {
        "shaders/corn/prepass_instanced_vertex.wgsl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.vertex.shader_defs.push(ShaderDefVal::Bool("CORN_INSTANCED".to_string(), true));
        descriptor.vertex.buffers.push(VertexBufferLayout{
            array_stride: CORN_DATA_SIZE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vertex_attr_array![8 => Float32x4, 9 => Float32x2, 10 => Uint32x2].to_vec()
        });
        // Necessary, fixes bug with standard shader
        // TODO: The push constant could already be added by this point, in my testing it wasn't, but it could change dependening on context
        // I should probably have some sort of test to only add it if there isn't already a push constant added
        descriptor.push_constant_ranges.push(PushConstantRange{stages: ShaderStages::VERTEX, range: 0..4});
        Ok(())
    }
}

pub struct DrawCorn;
impl<P: PhaseItem> RenderCommand<P> for DrawCorn {
    type Param = (SRes<RenderAssets<Mesh>>, SRes<RenderMeshInstances>, SRes<CornInstanceBuffer>);
    type ViewWorldQuery = ();
    type ItemWorldQuery = ();
    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: (),
        (meshes, mesh_instances, corn_instance_buffer): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        if !corn_instance_buffer.ready_to_render() {return RenderCommandResult::Success;}
        let meshes = meshes.into_inner();
        let mesh_instances = mesh_instances.into_inner();
        let corn_instance_buffer = corn_instance_buffer.into_inner();

        let Some(mesh_instance) = mesh_instances.get(&item.entity()) else {
            return RenderCommandResult::Failure;
        };
        let Some(gpu_mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Failure;
        };
        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, corn_instance_buffer.get_sorted_buffer().unwrap().slice(..));
        let indirect_buffer = corn_instance_buffer.get_indirect_buffer().unwrap();

        let batch_range = item.batch_range();
        pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            &(batch_range.start as i32).to_le_bytes(),
        );
        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count: _,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                for i in 0..corn_instance_buffer.lod_count{
                    pass.draw_indexed_indirect(indirect_buffer, (i*20).into());
                }
            }
            GpuBufferInfo::NonIndexed => {
                pass.multi_draw_indirect(indirect_buffer, 0, corn_instance_buffer.lod_count as u32);
            }
        }
        RenderCommandResult::Success
    }
}

pub fn corn_mesh_loaded(corn_meshes: Res<CornMeshes>) -> bool{
    corn_meshes.loaded
}
/// Spawns a renderable corn stalk in the center of the screen which will act as our corn field
/// Our extended material and specialized material plugin override the render logic to instance the stalk into our corn fields
pub fn spawn_corn_anchor(
    mut commands: Commands,
    std_materials: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<CornMaterial>>,
    corn_meshes: Res<CornMeshes>
){
    if let Some(mat) = std_materials.get(corn_meshes.materials.get(&"CornLeaves".to_string()).unwrap().clone()){
        commands.spawn((MaterialMeshBundle::<CornMaterial>{
            mesh: corn_meshes.global_mesh.as_ref().unwrap().clone(),
            //material: materials.add(CornMaterial{base: mat.clone(), extension: CornMaterialExtension::default()}),
            material: materials.add(CornMaterial{base: mat.clone(), extension: CornMaterialExtension{}}),
            ..Default::default()
        }, NoFrustumCulling, NoAutomaticBatching{}));
    }    
}
/// ### Adds the vote scan compact prepass functionality to the game
pub struct CornRenderPlugin;
impl Plugin for CornRenderPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins(SpecializedMaterialPlugin::<CornMaterial, CornDrawRender, CornDrawPrepass>::default())
            .add_systems(Update, spawn_corn_anchor.run_if(corn_mesh_loaded.and_then(run_once())));
    }
}

