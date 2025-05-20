pub mod shader;
pub mod init;
pub mod scan_prepass;
pub mod asset;
pub mod render;

use bevy::{prelude::*, render::{
    batching::NoAutomaticBatching, extract_component::{ExtractComponent, ExtractComponentPlugin}, extract_resource::{ExtractResource, ExtractResourcePlugin}, render_resource::*, renderer::RenderDevice, view::NoFrustumCulling, Render, RenderApp, RenderSet
}};
use bytemuck::{Pod, Zeroable};
use init::{simple::SimpleInitShader, CornInitializationPlugin};
use asset::CornModelPlugin;
use render::CornRenderPlugin;
use scan_prepass::ScanPrepassPlugin;
use crate::{scenes::lobby::LobbyScene, systems::{scenes::OnSpawnScene, util::default_resources::SimpleMaterials}, util::observer_ext::ObserverParent};

pub const LOD_COUNT: u32 = 5;

/// Struct representing the Per Corn Stalk data on  the GPU
#[derive(Default, Clone, Copy, Pod, Zeroable, Debug, ShaderType, PartialEq, Reflect)]
#[repr(C)]
pub struct CornData{
    /// Offset from the origin for this piece of corn.
    offset: Vec3,
    /// Scale of this corn stalk
    scale: f32,
    /// Rotation of this corn stalk in the form <sin(theta), cos(theta)>
    rotation: Vec2,
    /// an id, not used by most corn fields, but can be used to signify special traits
    uuid: u32,
    /// whether or not the corn piece should be rendered
    enabled: u32
}
impl CornData{
    pub const DATA_SIZE: u64 = 32;
    pub const VERTEX_DATA_SIZE: u64 = 64;
}

/// Top level Tag Component for Corn Fields. 
/// Each entity with a CornField and CornPositionInitializer Component has a corresponding Buffer of corn stalk instances in the render app.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
#[require(Transform, Visibility, NoFrustumCulling, NoAutomaticBatching(|| NoAutomaticBatching))]
pub struct CornField;

/// Global resource for lod cutoffs
#[derive(Debug, Clone, Reflect, Resource, ExtractResource)]
#[reflect(Resource)]
pub struct GlobalLodCutoffs(pub [f32; LOD_COUNT as usize]);
impl Default for GlobalLodCutoffs{
    fn default() -> Self {
        let mut cutoffs = [0.0; LOD_COUNT as usize];
        for i in 0..LOD_COUNT {
            cutoffs[i as usize] = 2_i32.pow(i) as f32 * 5.0;
        }
        Self(cutoffs)
    }
}

/// Common shader files used in all shaders
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
#[reflect(Resource)]
pub struct CornCommonShader(pub Vec<Handle<Shader>>);
impl FromWorld for CornCommonShader{
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        Self(vec![
            server.load("shaders/noise.wgsl"),
            server.load("shaders/corn/render/wind.wgsl"),
            server.load("shaders/corn/corn_common.wgsl"),
        ])
    }
}

/// Tag component for Corn Fields that are fully initialized
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct CornLoaded;

/// Component for Corn Fields containing the instance buffer
#[derive(Debug, Clone, Component)]
pub struct InstanceBuffer(pub Buffer, pub u64);
impl InstanceBuffer{
    pub fn create_buffer(label: String, count: u64, render_device: &RenderDevice) -> Self{
        Self(render_device.create_buffer(&BufferDescriptor{
            label: Some(label.as_str()),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            size: count*CornData::DATA_SIZE,
            mapped_at_creation: false
        }), count)
    }
    pub fn create_buffer_with_data(label: String, render_device: &RenderDevice, data: &[u8]) -> Self{
        Self(render_device.create_buffer_with_data(&BufferInitDescriptor { 
            label: Some(label.as_str()), 
            contents: data, 
            usage:  BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        }), data.len() as u64/CornData::DATA_SIZE)
    }
}

/// Component for Corn Fields containing the indirect buffer
#[derive(Debug, Clone, Component)]
pub struct IndirectBuffer(pub Buffer);
impl IndirectBuffer{
    // System which creates indirect buffers for loaded corn field
    fn spawn_indirect(
        query: Query<Entity, (With<CornLoaded>, Without<Self>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for entity in query.iter(){
            // TODO: Get this working.
            //let data: Vec<u32> = corn_meshes.lod_data.iter().map(|lod| 
            //    [lod.total_vertices as u32, 0, lod.start_vertex as u32, 0, 0]
            //).collect();
            let indirect_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor { 
                label: Some("Corn Field Indirect Buffer"), 
                contents: &[1_u8; 20*LOD_COUNT as usize], 
                usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_SRC
            });
            commands.entity(entity).insert(IndirectBuffer(indirect_buffer));
        }
    }
}

/// Component for Corn Fields containing the vertex instance buffer
#[derive(Debug, Clone, Component)]
pub struct VertexInstanceBuffer(pub Buffer);
impl VertexInstanceBuffer{
    fn spawn_vertex_buffer(
        query: Query<(Entity, &InstanceBuffer), (With<CornLoaded>, Without<Self>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, InstanceBuffer(_, count)) in query.iter(){
            commands.entity(entity).insert(VertexInstanceBuffer(render_device.create_buffer(&BufferDescriptor{
                label: Some("Corn Field Vertex Instance Buffer"),
                size: count*64,
                usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
                mapped_at_creation: false
            })));
        }
    }
}

/// Parent entity for all corn field observers
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct CornFieldObserver;
impl ObserverParent for CornFieldObserver{
    fn get_name(&self) -> Name {
        Name::from("Corn Field Observers")
    }
}

/// Adds all corn field functionality to the app
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<CornField>()
            .register_type::<CornLoaded>()
            .register_type::<CornData>()

            .add_plugins(ExtractComponentPlugin::<CornField>::default())

            .register_type::<GlobalLodCutoffs>()
            .init_resource::<GlobalLodCutoffs>()
            .add_plugins(ExtractResourcePlugin::<GlobalLodCutoffs>::default())

            .register_type::<CornCommonShader>()
            .init_resource::<CornCommonShader>()

            .sub_app_mut(RenderApp).add_systems(Render, (
                IndirectBuffer::spawn_indirect, VertexInstanceBuffer::spawn_vertex_buffer
            ).in_set(RenderSet::PrepareResources));
        app.add_plugins((CornInitializationPlugin, ScanPrepassPlugin, CornModelPlugin, CornRenderPlugin));

        app.add_systems(OnSpawnScene(LobbyScene), test_init);
    }
}

#[derive(Clone, Component, Default, Debug, Reflect)]
pub struct CornSensor{
    pub is_in_corn: f32
}

pub fn test_init(
    mut commands: Commands,
    default_resources: Res<SimpleMaterials>
){
    commands.spawn((
        CornField,
        SimpleInitShader::new(
            Vec3::ZERO, 
            Vec2::ONE*5.0, 
            UVec2::new(11, 11), 
            Vec2::new(0.9, 1.1), 
            0.0
        ),
        Transform::from_xyz(0.0, 2.0, 0.0),
        MeshMaterial3d(default_resources.red.clone())
    ));
}
