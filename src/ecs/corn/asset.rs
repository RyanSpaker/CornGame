use async_channel::Sender;
use bevy::{prelude::*, render::extract_resource::{ExtractResource, ExtractResourcePlugin}, utils::hashbrown::HashMap};
use crate::util::observer_ext::ObserveAsAppExt;

use super::{CornField, CornFieldObserver};

#[derive(Default, Debug)]
pub struct ConvertCornMeshError;
impl std::fmt::Display for ConvertCornMeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}
impl core::error::Error for ConvertCornMeshError{}

// Observer which attaches corn meshes to any corn field
pub fn attach_mesh(trigger: Trigger<OnAdd, CornField>, mut commands: Commands, model: Res<CornModel>){
    commands.entity(trigger.entity()).insert_if_new(Mesh3d(model.mesh_handle.clone()));
}

/// Component which is used to send corn meshes to an async load function for the corn mesh asset. This way the handle for the mesh can be created before the gltf has loaded
#[derive(Debug, Clone, Component)]
pub struct CornMeshSender(pub Handle<Gltf>, pub Sender<Vec<Vec<Mesh>>>);

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource, ExtractResource)]
#[reflect(Resource)]
pub struct CornModel{
    gltf_handle: Handle<Gltf>,
    pub mesh_handle: Handle<Mesh>,
    // List of (# of vtcs, start vtx) for each lod.
    pub lod_info: Vec<(usize, usize)>
}
impl CornModel{
    // Whenever GLTF loads, this runs, and checks to see if it was the corn model. If so we recompute the mesh
    fn on_load_gltf(
        mut event_reader: EventReader<AssetEvent<Gltf>>,
        mut resource: ResMut<Self>,
        gltf_assets: Res<Assets<Gltf>>,
        mut scene_assets: ResMut<Assets<Scene>>,
        mesh_assets: Res<Assets<Mesh>>,
        asset_server: Res<AssetServer>,
        mut senders: Query<(Entity, &mut CornMeshSender)>,
        mut commands: Commands
    ){
        for event in event_reader.read(){
            if !event.is_loaded_with_dependencies(&resource.gltf_handle) {continue;}
            // Grab GLTF Scene
            let Some(scene) = gltf_assets.get(resource.gltf_handle.id())
                .and_then(|gltf| gltf.scenes.get(0))
                .and_then(|scene| scene_assets.get_mut(scene)) 
            else {continue;};
            let world = &mut scene.world;
            // Find the meshes
            let mut mesh_query = world.query::<(&Parent, &Mesh3d)>();
            let meshes: Vec<(Handle<Mesh>, Entity)> = mesh_query.iter(world)
                .map(|(parent, mesh3d)| (mesh3d.0.clone(), parent.get())).collect();
            // Group meshes by their parents parent. 
            let mut parent_query = world.query::<&Parent>();
            let mut lods: HashMap<Entity, Vec<Handle<Mesh>>> = HashMap::default();
            for (mesh, parent) in meshes.into_iter(){
                let Ok(middle) = parent_query.get(world, parent) else {continue;};
                if let Some(list) = lods.get_mut(&middle.get()) {
                    list.push(mesh);
                } else {
                    lods.insert(middle.get(), vec![mesh]);
                }
            }
            // Get Mesh Pointers
            let mut lods: Vec<(usize, Vec<&Mesh>)> = lods.into_values().map(|lod| {
                let mesh_pointers: Vec<&Mesh> = lod.into_iter().map(|handle| mesh_assets.get(&handle).unwrap()).collect();
                let size = mesh_pointers.iter().map(|mesh| mesh.count_vertices()).sum();
                (size, mesh_pointers)
            }).collect();
            // Sort lods
            lods.sort_by(|(a, _), (b, _)| b.cmp(a));
            // Get Vertex counts
            let vertex_counts: Vec<usize> = lods.iter().map(|(count, _)| *count).collect();
            let lod_data = vertex_counts.iter().fold((vec![], 0), | (mut lod_data, sum), val| {
                lod_data.push((*val, sum));
                (lod_data, sum+val)
            }).0;
            resource.lod_info = lod_data;
            // Clone meshes
            let lods = lods.into_iter().map(|(_, lod)| lod.into_iter().cloned().collect()).collect();
            // Send Mesh, or Queue its creation
            if let Some((entity, sender)) = senders.iter_mut().find(|(_, s)| s.0 == resource.gltf_handle) {
                let _ = sender.1.force_send(lods);
                sender.1.close();
                commands.entity(entity).despawn();
            } else {
                resource.mesh_handle = asset_server.add_async(Self::convert_gltf(lods));
            }
        }
    }
    // Given a vec of vec of meshes, create the merged final mesh
    async fn convert_gltf(meshes: Vec<Vec<Mesh>>) -> Result<Mesh, ConvertCornMeshError>{
        let meshes = meshes;
        let mut lods: Vec<(usize, Vec<&Mesh>)> = meshes.iter().map(|lod| {
            let count = lod.iter().map(|mesh| mesh.count_vertices()).sum();
            (count, lod.iter().collect::<Vec<&Mesh>>())
        }).collect();
        lods.sort_by(|(a, _), (b, _)| {b.cmp(a)});
        let mut iter = lods.into_iter().map(|(_, lod)| lod.into_iter()).flatten();
        let mut merged = iter.next().unwrap().clone();
        for mesh in iter {merged.merge(mesh);}
        Ok(merged)
    }
}
impl FromWorld for CornModel{
    // Loads the gltf, and creates a handle for the merged mesh
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        let gltf_handle = assets.load("models/CornTest.glb");
        let (tx, rx) = async_channel::bounded(1);
        world.spawn(CornMeshSender(gltf_handle.clone(), tx));
        let assets = world.resource::<AssetServer>();
        let mesh_handle = assets.add_async(async move {
            let meshes = rx.recv().await.map_err(|_| ConvertCornMeshError)?;
            Self::convert_gltf(meshes).await
        });
        
        Self{ gltf_handle, mesh_handle, lod_info: vec![] }
    }
}

pub struct CornModelPlugin;
impl Plugin for CornModelPlugin{
    fn build(&self, app: &mut App) {
        app.register_type::<CornModel>()
            .add_plugins(ExtractResourcePlugin::<CornModel>::default())
            .add_systems(Startup,
                CornModel::on_load_gltf.run_if(on_event::<AssetEvent<Gltf>>)
            )
            .add_systems(Update,
                CornModel::on_load_gltf.run_if(on_event::<AssetEvent<Gltf>>)
            )
            .add_observer_as(attach_mesh, CornFieldObserver);
    }
    fn finish(&self, app: &mut App) {
        app.init_resource::<CornModel>();
    }
}

