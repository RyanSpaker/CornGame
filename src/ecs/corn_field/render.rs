use bevy::{
    pbr::{ExtendedMaterial, StandardMaterial, MaterialExtension, RenderMeshInstances}, 
    render::{render_resource::{AsBindGroup, ShaderDefVal, VertexBufferLayout}, render_phase::{PhaseItem, RenderCommand, TrackedRenderPass, RenderCommandResult}, render_asset::RenderAssets, mesh::{Mesh, GpuBufferInfo}}, 
    asset::Asset, 
    reflect::Reflect, ecs::system::{lifetimeless::SRes, SystemParamItem}
};
use wgpu::{VertexFormat, VertexAttribute, vertex_attr_array, ShaderStages, PushConstantRange};
use super::{CORN_DATA_SIZE, CornInstanceBuffer};

pub type CornMaterial = ExtendedMaterial<StandardMaterial, CornMaterialExtension>;

#[derive(Default, Debug, Clone, AsBindGroup, Asset, Reflect)]
pub struct CornMaterialExtension{}
impl MaterialExtension for CornMaterialExtension{
    fn vertex_shader() -> bevy::render::render_resource::ShaderRef {
        "shaders/corn/instanced_vertex.wgsl".into()
    }
/*
    fn prepass_vertex_shader() -> bevy::render::render_resource::ShaderRef {
        "shaders/corn/prepass_instanced_vertex.wgsl".into()
    }

    fn deferred_vertex_shader() -> bevy::render::render_resource::ShaderRef {
        "shaders/corn/prepass_instanced_vertex.wgsl".into()
    }*/

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        println!("{}", descriptor.vertex.buffers.len());
        descriptor.vertex.shader_defs.push(ShaderDefVal::Bool("CORN_INSTANCED".to_string(), true));
        descriptor.vertex.buffers.push(VertexBufferLayout{
            array_stride: CORN_DATA_SIZE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vertex_attr_array![8 => Float32x4, 9 => Float32x2, 10 => Uint32x2].to_vec()
        });
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
        pass.set_vertex_buffer(1, corn_instance_buffer.sorted_buffer.as_ref().unwrap().slice(..));
        let indirect_buffer = corn_instance_buffer.indirect_buffer.as_ref().unwrap();
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
                count,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                //pass.draw_indexed(0..*count, 0, 0..2);
                pass.multi_draw_indexed_indirect(
                    indirect_buffer, 
                    0, 
                    corn_instance_buffer.lod_count.clone()
                );
            }
            GpuBufferInfo::NonIndexed => {
                pass.multi_draw_indirect(indirect_buffer, 0, corn_instance_buffer.lod_count.clone());
            }
        }
        RenderCommandResult::Success
    }
}


pub struct DrawMesh2;
impl<P: PhaseItem> RenderCommand<P> for DrawMesh2 {
    type Param = (SRes<RenderAssets<Mesh>>, SRes<RenderMeshInstances>, SRes<CornInstanceBuffer>);
    type ViewWorldQuery = ();
    type ItemWorldQuery = ();
    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: (),
        (meshes, mesh_instances, instance_buffer): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let meshes = meshes.into_inner();
        let mesh_instances = mesh_instances.into_inner();
        let corn = instance_buffer.into_inner();

        if !corn.ready_to_render() {return RenderCommandResult::Success;}

        let Some(mesh_instance) = mesh_instances.get(&item.entity()) else {
            return RenderCommandResult::Failure;
        };
        let Some(gpu_mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Failure;
        };

        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, corn.sorted_buffer.as_ref().unwrap().slice(..));

        let batch_range = item.batch_range();
        #[cfg(all(feature = "webgl", target_arch = "wasm32"))]
        pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            &(batch_range.start as i32).to_le_bytes(),
        );
        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                pass.draw_indexed(0..*count, 0, 0..1);
            }
            GpuBufferInfo::NonIndexed => {
                pass.draw(0..gpu_mesh.vertex_count, batch_range.clone());
            }
        }
        RenderCommandResult::Success
    }
}


pub struct DrawMeshTwice;
impl<P: PhaseItem> RenderCommand<P> for DrawMeshTwice {
    type Param = (SRes<RenderAssets<Mesh>>, SRes<RenderMeshInstances>);
    type ViewWorldQuery = ();
    type ItemWorldQuery = ();
    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: (),
        (meshes, mesh_instances): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let meshes = meshes.into_inner();
        let mesh_instances = mesh_instances.into_inner();

        let Some(mesh_instance) = mesh_instances.get(&item.entity()) else {
            return RenderCommandResult::Failure;
        };
        let Some(gpu_mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Failure;
        };

        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));

        let batch_range = item.batch_range();
        #[cfg(all(feature = "webgl", target_arch = "wasm32"))]
        pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            &(batch_range.start as i32).to_le_bytes(),
        );
        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                pass.draw_indexed(0..*count, 0, batch_range.clone());
            }
            GpuBufferInfo::NonIndexed => {
                pass.draw(0..gpu_mesh.vertex_count, batch_range.clone());
            }
        }
        RenderCommandResult::Success
    }
}

