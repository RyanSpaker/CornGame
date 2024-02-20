use crate::{
    ecs::corn::{
        asset::{CornAsset, CornModel}, buffer::{CornInstanceBuffer, CORN_DATA_SIZE}
    }, util::specialized_material::{
        SpecializedDrawMaterial, SpecializedDrawPrepass, SpecializedMaterialPlugin
    }
};
use bevy::{
    asset::{Asset, Assets}, ecs::system::{lifetimeless::SRes, Commands, Res, ResMut, SystemParamItem}, pbr::{
        DirectionalLightShadowMap, ExtendedMaterial, MaterialExtension, MaterialMeshBundle, RenderMeshInstances, StandardMaterial
    }, prelude::*, reflect::Reflect, render::{
        batching::NoAutomaticBatching, globals::GlobalsUniform, mesh::{GpuBufferInfo, Mesh}, render_asset::RenderAssets, render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass}, render_resource::{AsBindGroup, AsBindGroupShaderType, ShaderDefVal, VertexBufferLayout}, view::NoFrustumCulling, Render, RenderSet
    }, scene::ron::de
};
use wgpu::{vertex_attr_array, PushConstantRange, ShaderStages};

/// Corn rendering uses a Special Material which expands upon the `StandardMaterial` adding instancing support.
/// We add this material to the app with a special Material Plugin called `SpecializedMaterialPlugin`, which allows us to override the Draw commands used by the Material.
///
/// This makes it so that we can Draw the corn instanced, while using the Standard Material by remaking the vertex shader, and overriding the draw command.
///
/// In order to actually draw the corn, we spawn a single Corn stalk with the master corn mesh in the middle of the scene. When the app tries to render this object
/// our custom draw commands are called, which obtain the corn instance buffer, and draw our corn instanced.

mod shaders {
    pub const INSTANCED_VERTEX: &str = "shaders/corn/render/instanced_vertex.wgsl";
    pub const PREPASS_INSTANCED_VERTEX: &str = "shaders/corn/render/prepass_instanced_vertex.wgsl";
}

/// The material type of the corn anchor asset
pub type CornMaterial = ExtendedMaterial<StandardMaterial, CornMaterialExtension>;
/// The render draw command used by the corn
pub type CornDrawRender = SpecializedDrawMaterial<CornMaterial, DrawCorn>;
/// The prepass draw command used by the corn
pub type CornDrawPrepass = SpecializedDrawPrepass<CornMaterial, DrawCorn>;

/// A material extension for the corn. Adds our instance buffer as a vertex buffer,
/// sets up a push constant which will hold our meshes instance id (bevy puts all meshes into a big buffer, this is the index into that buffer containing our mesh)
/// adds a shaderdef enabling our instanced code
#[derive(Default, Clone, AsBindGroup, Asset, Reflect)]
pub struct CornMaterialExtension{
    // #[uniform(100)]
    // pub time: f32,
}

// pub fn update_time(
//     time: Res<Time>,
//     mut assets: ResMut<Assets<CornMaterial>>
// ){
//     for (_, mat) in assets.iter_mut(){
//         mat.extension.time = time.elapsed_seconds()
//     }
// }

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
        pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor
            .vertex
            .shader_defs
            .push(ShaderDefVal::Bool("CORN_INSTANCED".to_string(), true));
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: CORN_DATA_SIZE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vertex_attr_array![8 => Float32x4, 9 => Float32x2, 10 => Uint32x2].to_vec(),
        });
        // Necessary, fixes bug with standard shader
        // TODO: The push constant could already be added by this point, in my testing it wasn't, but it could change dependening on context
        // I should probably have some sort of test to only add it if there isn't already a push constant added
        descriptor.push_constant_ranges.push(PushConstantRange {
            stages: ShaderStages::VERTEX,
            range: 0..8,
        });
        Ok(())
    }
}

/// This is the command whihc will be called instead of DrawMesh. Does essentially the same stuff but gets our instance buffer and draws with an isntanced draw command instead of a normal one.
pub struct DrawCorn;
impl<P: PhaseItem> RenderCommand<P> for DrawCorn {
    type Param = (
        SRes<Time>,
        SRes<RenderAssets<Mesh>>,
        SRes<RenderMeshInstances>,
        SRes<CornInstanceBuffer>,
    );
    type ViewQuery = ();
    type ItemQuery = ();
    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (time, meshes, mesh_instances, corn_instance_buffer): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        if !corn_instance_buffer.ready_to_render() {
            return RenderCommandResult::Success;
        }
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
        pass.set_vertex_buffer(
            1,
            corn_instance_buffer.get_sorted_buffer().unwrap().slice(..),
        );
        let indirect_buffer = corn_instance_buffer.get_indirect_buffer().unwrap();

        let batch_range = item.batch_range();
        pass.set_push_constants(
            ShaderStages::VERTEX,
            0,
            &(batch_range.start as i32).to_le_bytes(),
        );
        
        pass.set_push_constants(ShaderStages::VERTEX, 4, &time.elapsed_seconds().to_le_bytes());

        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count: _,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                for i in 0..corn_instance_buffer.lod_count {
                    pass.draw_indexed_indirect(indirect_buffer, (i * 20).into());
                }
            }
            GpuBufferInfo::NonIndexed => {
                pass.multi_draw_indirect(indirect_buffer, 0, corn_instance_buffer.lod_count as u32);
            }
        }
        RenderCommandResult::Success
    }
}

/// Spawns a renderable corn stalk in the center of the screen which will act as our corn field
/// Our extended material and specialized material plugin override the render logic to instance the stalk into our corn fields
pub fn spawn_corn_anchor(
    mut commands: Commands,
    std_materials: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<CornMaterial>>,
    corn: Res<CornModel>,
    corn_asset: Res<Assets<CornAsset>>,
    mut events: EventReader<AssetEvent<CornAsset>>,
) {
    if !events
        .read()
        .any(|event| event.is_loaded_with_dependencies(corn.asset.clone()))
    {
        return;
    }
    let corn_meshes = corn_asset.get(corn.asset.clone()).unwrap();
    if let Some(mat) = std_materials.get(
        corn_meshes
            .materials
            .get(&"CornLeaves".to_string())
            .unwrap()
            .clone(),
    ) {
        commands.spawn((
            MaterialMeshBundle::<CornMaterial> {
                mesh: corn_meshes.master_mesh.clone(),
                material: materials.add(CornMaterial {
                    base: mat.clone(),
                    extension: CornMaterialExtension::default(),
                }),
                //material: std_materials.add(StandardMaterial::from(Color::RED)),
                ..default()
            },
            NoFrustumCulling,
            NoAutomaticBatching {},
        ));
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
            .add_systems(
                Update,
                spawn_corn_anchor.run_if(on_event::<AssetEvent<CornAsset>>()),
            );
        //.insert_resource(DirectionalLightShadowMap { size: 4096 });
        // TODO heirarchical shadow maps? antialiasing?
    }
}