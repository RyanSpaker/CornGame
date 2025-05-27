use crate::util::{observer_ext::ObserveAsAppExt, specialized_material::{SpecializedDrawMaterial, SpecializedDrawPrepass, SpecializedMaterialPlugin}};
use super::{CornData, CornField, CornFieldObserver, CornLoaded, IndirectBuffer, VertexInstanceBuffer, LOD_COUNT};
use bevy::{
    asset::Asset, ecs::{query::ROQueryItem, system::{lifetimeless::{Read, SRes}, SystemParamItem}}, log::Level, pbr::{ExtendedMaterial, MaterialExtension, RenderMeshInstances, StandardMaterial}, prelude::*, reflect::Reflect, render::{
        mesh::{allocator::MeshAllocator, RenderMesh, RenderMeshBufferInfo}, render_asset::RenderAssets, render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass}, render_resource::{AsBindGroup, ShaderDefVal, VertexBufferLayout}
    }, utils::tracing::event
};
use wgpu::{vertex_attr_array, IndexFormat, PushConstantRange, ShaderStages};

/// Corn rendering uses a Special Material which expands upon the `StandardMaterial` adding instancing support.
/// We add this material to the app with `SpecializedMaterialPlugin`, which allows us to override the Draw commands used by the Material.
///
/// This makes it so that we can Draw the corn instanced, while using the Standard Material by remaking the vertex shader, and overriding the draw command.

mod shaders {
    pub const INSTANCED_VERTEX: &str = "shaders/corn/render/vertex.wgsl";
    pub const PREPASS_INSTANCED_VERTEX: &str = "shaders/corn/render/prepass.wgsl";
}

/// The material type of the corn anchor asset
pub type CornMaterial = ExtendedMaterial<StandardMaterial, CornMaterialExtension>;
/// The render draw command used by the corn
pub type CornDrawRender = SpecializedDrawMaterial<CornMaterial, DrawCorn>;
/// The prepass draw command used by the corn
pub type CornDrawPrepass = SpecializedDrawPrepass<CornMaterial, DrawCorn>;

/// Automatically replaces std materials on corn fields with Corn Materials
pub fn replace_standard_materials(
    trigger: Trigger<OnInsert, MeshMaterial3d<StandardMaterial>>,
    query: Query<(Entity, &MeshMaterial3d<StandardMaterial>), (With<CornField>, Without<MeshMaterial3d<CornMaterial>>)>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    std_mats: Res<Assets<StandardMaterial>>
){
    let Ok((entity, material)) = query.get(trigger.entity()) else {return;};
    let Some(material) = std_mats.get(material.id()) else {error!("Std Material on Corn Field is not Loaded, wierd"); return;};
    let extd_mat = ExtendedMaterial{base: material.clone(), extension: CornMaterialExtension{}};
    let handle = assets.add(extd_mat);
    commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(handle));
}

pub trait ExtendWithCornMaterial: Material{fn extend_with_corn(self) -> ExtendedMaterial<Self, CornMaterialExtension>;}
impl<M: Material> ExtendWithCornMaterial for M {
    fn extend_with_corn(self) -> ExtendedMaterial<Self, CornMaterialExtension> {
        ExtendedMaterial { base: self, extension: CornMaterialExtension {} }
    }
}

/// A material extension for the corn. Adds our instance buffer as a vertex buffer,
/// adds a shaderdef enabling our instanced code
#[derive(Default, Clone, AsBindGroup, Asset, Reflect)]
pub struct CornMaterialExtension{}
impl MaterialExtension for CornMaterialExtension {
    fn vertex_shader() -> bevy::render::render_resource::ShaderRef {
        shaders::INSTANCED_VERTEX.into()
    }
    fn prepass_vertex_shader() -> bevy::render::render_resource::ShaderRef {
        shaders::PREPASS_INSTANCED_VERTEX.into()
    }
    fn deferred_vertex_shader() -> bevy::render::render_resource::ShaderRef {
        shaders::PREPASS_INSTANCED_VERTEX.into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor
            .vertex
            .shader_defs
            .push(ShaderDefVal::Bool("CORN_INSTANCED".to_string(), true));
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: CornData::VERTEX_DATA_SIZE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vertex_attr_array![8 => Float32x4, 9 => Float32x4, 10 => Float32x4, 11 => Float32x4].to_vec(),
        });
        descriptor.push_constant_ranges.push(PushConstantRange{stages: ShaderStages::VERTEX, range: 0..4});
        Ok(())
    }
}

pub struct DrawCorn;
impl<P: PhaseItem> RenderCommand<P> for DrawCorn {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = (Read<VertexInstanceBuffer>, Read<IndirectBuffer>, Has<CornLoaded>);
    #[inline]
    fn render<'w>(
        item: &P,
        _: ROQueryItem<Self::ViewQuery>,
        entity_query: Option<ROQueryItem<'w, Self::ItemQuery>>,
        (meshes, mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some((VertexInstanceBuffer(instance_buffer), IndirectBuffer(indirect_buffer), true)) = entity_query.as_ref() 
        else {return RenderCommandResult::Skip;};

        let meshes = meshes.into_inner();
        let mesh_instances = mesh_instances.into_inner();
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_instance) = mesh_instances.render_mesh_queue_data(item.main_entity()) else {
            return RenderCommandResult::Failure("unknown");
        };
        let mesh_asset_id = mesh_instance.mesh_asset_id;

        let Some(gpu_mesh) = meshes.get(mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[item.batch_range().start]));

        // Draw either directly or indirectly, as appropriate.
        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format, ..
            } => {
                let Some(index_buffer_slice) = mesh_allocator.mesh_index_slice(&mesh_asset_id)
                else {return RenderCommandResult::Skip;};
                let (start, end) = match index_format{
                    IndexFormat::Uint16 => {
                        (index_buffer_slice.range.start as u64*2, index_buffer_slice.range.end as u64*2)
                    },
                    IndexFormat::Uint32 => {
                        (index_buffer_slice.range.start as u64*4, index_buffer_slice.range.end as u64*4)
                    }
                };
                pass.set_index_buffer(index_buffer_slice.buffer.slice(start..end), 0, *index_format);
                event!(Level::TRACE, "Rendering Corn, indexed: {}", true);
                pass.multi_draw_indexed_indirect(indirect_buffer, 0, LOD_COUNT);
            }
            RenderMeshBufferInfo::NonIndexed => {
                event!(Level::TRACE, "Rendering Corn, indexed: {}", false);
                pass.multi_draw_indirect(indirect_buffer, 0, LOD_COUNT);
            }
        }
        RenderCommandResult::Success
    }
}

///Adds corn rendering functionality to the game
pub struct CornRenderPlugin;
impl Plugin for CornRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpecializedMaterialPlugin::<
            CornMaterial,
            CornDrawRender,
            CornDrawPrepass,
        >::default())
        .add_observer_as(replace_standard_materials, CornFieldObserver);
    }
}