use bevy::{asset::LoadedAsset, ecs::entity::EntityHashSet, gltf::{GltfMesh, GltfNode}, prelude::*};
use wgpu::core::resource::ParentDevice;

/// Component attached to corn models that describes the lod info necessary for the indirect buffer
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Component)]
#[reflect(Component)]
pub struct CornLodInfo{
    // List of (# of vtcs, start vtx) for each lod.
    pub lods: Vec<(usize, usize)>
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
#[reflect(Resource)]
pub struct CornModel{
    gltf_handle: Handle<Gltf>,
    pub mesh_handle: Option<Handle<Mesh>>,
    // List of (# of vtcs, start vtx) for each lod.
    pub lod_info: Vec<(usize, usize)>
}
impl CornModel{
    fn convert_gltf(
        mut event_reader: EventReader<AssetEvent<Gltf>>,
        mut resource: ResMut<Self>,
        gltf_assets: Res<Assets<Gltf>>,
        gltf_mesh_assets: Res<Assets<GltfMesh>>,
        gltf_node_assets: Res<Assets<GltfNode>>,
        mut scene_assets: ResMut<Assets<Scene>>,
        mesh_assets: Res<Assets<Mesh>>
    ){
        for event in event_reader.read(){
            match event {
                AssetEvent::LoadedWithDependencies { id } => {
                    if resource.gltf_handle.id() == *id {
                        let Some(gltf) = gltf_assets.get(*id) else {continue;};
                        for scene in gltf.scenes.iter(){
                            let Some(scene) = scene_assets.get_mut(scene) else {continue;};
                            let world = &mut scene.world;
                            let mut query = world.query_filtered::<&Parent, With<Children>>();
                            let mut parents = EntityHashSet::default();
                            for parent in query.iter(world){
                                parents.insert(parent.get());
                            }
                            println!("Scene: {}", parents.len());
                        }
                        for node in gltf.nodes.iter(){
                            let Some(node) = gltf_node_assets.get(node) else {continue;};
                            println!("GLTF NODE: {} {} {}", node.name, node.children.len(), node.mesh.is_some());
                        }
                        for (name, mesh) in gltf.named_meshes.iter(){
                            let Some(mesh) = gltf_mesh_assets.get(mesh) else {continue;};
                            println!("GLTF MESH: {}", name);
                            for primitive in mesh.primitives.iter() {
                                let Some(mesh) = mesh_assets.get(&primitive.mesh) else {continue;};
                                println!("Primitive Name: {}", primitive.name);
                            }
                        }
                    }
                },
                _ => {}
            }
        }
    }
}
impl FromWorld for CornModel{
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        let gltf_handle = assets.load("models/Corn.glb");
        Self{ gltf_handle, mesh_handle: None, lod_info: vec![] }
    }
}

pub struct CornModelPlugin;
impl Plugin for CornModelPlugin{
    fn build(&self, app: &mut App) {
        app.register_type::<CornModel>()
            .add_systems(Update, CornModel::convert_gltf.run_if(on_event::<AssetEvent<Gltf>>));
    }
    fn finish(&self, app: &mut App) {
        app.init_resource::<CornModel>();
    }
}

