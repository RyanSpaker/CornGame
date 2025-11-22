use std::sync::atomic::Ordering;
use crate::util::{specialized_material::{SpecializedDrawMaterial, SpecializedDrawPrepass, SpecializedMaterialPlugin}};
use super::{buffer::{CornData, IndirectBuffer, VertexInstanceBuffer}};
use bevy::{
    asset::Asset, ecs::{query::ROQueryItem, system::{SystemParamItem, lifetimeless::{Read, SRes}}}, pbr::{ExtendedMaterial, MaterialExtension, RenderMeshInstances, StandardMaterial}, prelude::*, reflect::Reflect, render::{
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator}, render_asset::RenderAssets, render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass}, render_resource::{AsBindGroup, AsBindGroupShaderType, ShaderDefVal, VertexBufferLayout}
    }
};
use wgpu::vertex_attr_array;

/// Corn rendering uses a Special Material which expands upon the `StandardMaterial` adding instancing support.
/// We add this material to the app with `SpecializedMaterialPlugin`, which allows us to override the Draw commands used by the Material.
///
/// This makes it so that we can Draw the corn instanced, while using the Standard Material by remaking the vertex shader, and overriding the draw command.

mod shaders {
    pub const INSTANCED_VERTEX: &str = "shaders/corn/render/vertex.wgsl";
    pub const PREPASS_INSTANCED_VERTEX: &str = "shaders/corn/render/prepass.wgsl";
}

/// The material type of the corn anchor asset
pub type StdCornMaterial = ExtendedMaterial<StandardMaterial, CornMaterialExtension>;
/// The render draw command used by the corn
pub type StdCornDrawRender = SpecializedDrawMaterial<StdCornMaterial, DrawCorn>;
/// The prepass draw command used by the corn
pub type StdCornDrawPrepass = SpecializedDrawPrepass<StdCornMaterial, DrawCorn>;

/// Trait to allow all materials to be extended with the CornMaterialExtension
pub trait ExtendWithCornMaterial: Material{fn extend_with_corn(self) -> ExtendedMaterial<Self, CornMaterialExtension>;}
impl<M: Material> ExtendWithCornMaterial for M {
    fn extend_with_corn(self) -> ExtendedMaterial<Self, CornMaterialExtension> {
        ExtendedMaterial { base: self, extension: CornMaterialExtension{ stress_vertex: false, wind: true, wind_normal: true, time: 1.0, fade_in: 1.0 } }
    }
}

/// A material extension for the corn. Adds our instance buffer as a vertex buffer,
/// adds a shaderdef enabling our instanced code
#[derive(Default, Clone, AsBindGroup, Asset, Reflect)]
#[bind_group_data(CornMaterialKey)]
pub struct CornMaterialExtension{
    pub wind: bool,
    pub wind_normal: bool,

    pub stress_vertex: bool,

    #[uniform(100)]
    pub time: f32,
   
    #[uniform(100)]
    pub fade_in: f32,
}

#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct CornMaterialKey {
    pub wind: bool,
    pub stress_vertex: bool,
    pub wind_normal: bool,
}

impl From<&CornMaterialExtension> for CornMaterialKey {
    fn from(material: &CornMaterialExtension) -> Self {
        Self {
            wind: material.wind,
            stress_vertex: material.stress_vertex,
            wind_normal: material.wind_normal,
        }
    }
}

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
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        dbg!(&descriptor.label);

        descriptor.primitive.cull_mode = None; // TODO how to get this value from StandardMaterial
        descriptor
            .vertex
            .shader_defs
            .push(ShaderDefVal::Bool("CORN_INSTANCED".to_string(), true));


            
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: CornData::VERTEX_DATA_SIZE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vertex_attr_array![8 => Float32x4, 9 => Float32x4, 10 => Float32x4, 11 => Float32x4].to_vec(),
        });

        // dbg!(&descriptor.vertex.buffers);

        if key.bind_group_data.wind {
            descriptor.vertex.shader_defs.push("WIND".into());
        }
        if key.bind_group_data.wind_normal {
            descriptor.vertex.shader_defs.push("WIND_NORMAL".into());
        }
        if key.bind_group_data.stress_vertex {
            descriptor.vertex.shader_defs.push("STRESS_VERTEX".into());
        }
        Ok(())
    }
}


pub struct DrawCorn;
impl<P: PhaseItem> RenderCommand<P> for DrawCorn {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>
    );
    type ViewQuery = ();
    type ItemQuery = (Read<VertexInstanceBuffer>, Read<IndirectBuffer>);
    #[inline]
    fn render<'w>(
        item: &P,
        _: ROQueryItem<Self::ViewQuery>,
        entity_query: Option<ROQueryItem<'w, Self::ItemQuery>>,
        (meshes, mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some((vib, indb)) = entity_query.as_ref() else {return RenderCommandResult::Skip;};
        
        if !vib.ready.load(Ordering::Relaxed) {return RenderCommandResult::Skip;}

        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_instance) = mesh_instances.render_mesh_queue_data(item.main_entity()) 
        else {return RenderCommandResult::Skip;};
        let Some(gpu_mesh) = meshes.get(mesh_instance.mesh_asset_id) 
        else {return RenderCommandResult::Skip;};
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id) 
        else {return RenderCommandResult::Skip;};
        let Some(index_buffer_slice) = mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id) 
        else {return RenderCommandResult::Failure("Corn failed to render as its mesh was not indexed");};

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, vib.buffer.slice(..));

        // Draw either directly or indirectly, as appropriate.
        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {index_format, ..} => {
                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), 0, *index_format);
                pass.multi_draw_indexed_indirect(&indb.buffer, 0, indb.lod_count as u32);
            }
            _ => {return RenderCommandResult::Failure("Corn failed to render, as its mesh is not indexed!")}
        }
        RenderCommandResult::Success
    }
}

///Adds corn rendering functionality to the game
pub struct CornRenderPlugin;
impl Plugin for CornRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpecializedMaterialPlugin::<
            StdCornMaterial,
            StdCornDrawRender,
            StdCornDrawPrepass,
        >::default());

        // app.register_type::<CornMaterialExtension>();
        // app.register_type::<StdCornMaterial>();
        // app.register_type::<MeshMaterial3d<StdCornMaterial>>();
        // app.register_type::<Handle<StdCornMaterial>>();
        app.register_asset_reflect::<StdCornMaterial>(); // actually this is what you need.
        app.add_systems(Update, |time: Res<Time>, mut materials: ResMut<Assets<StdCornMaterial>>| {
            for material in materials.iter_mut() {
                material.1.extension.time = time.elapsed_secs();
            }
        });
    }
}