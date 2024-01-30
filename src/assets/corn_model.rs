use bevy::{
    prelude::*, 
    gltf::{Gltf, GltfMesh}, 
    render::{
        RenderApp, 
        Extract, 
        render_resource::{PrimitiveTopology, VertexFormat}, 
        mesh::{Indices, VertexAttributeValues, MeshVertexAttribute}
    }, 
    tasks::{AsyncComputeTaskPool, Task}, utils::hashbrown::HashMap
};
use crate::core::loading::LoadingTaskCount;
use futures_lite::future;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum CornLoadState{
    #[default]
    PreLoad,
    Loading,
    Loaded,
    Unloading,
    Unloaded
}

#[derive(Resource, Clone)]
pub struct CornMeshes{
    pub lod_groups: Vec<Vec<(Handle<Mesh>, usize)>>,
    pub material_count: usize,
    pub material_names: HashMap<usize, String>,
    pub materials: HashMap<String, Handle<StandardMaterial>>,
    pub global_mesh: Option<Handle<Mesh>>,
    pub vertex_counts: Vec<(usize, Vec<usize>, usize)>,//start vertex, mesh piece vertex counts, total vertices
    pub lod_count: u32,
    pub loaded: bool
}
impl Default for CornMeshes{
    fn default() -> Self {
        Self{
            lod_groups: vec![], 
            global_mesh: None, 
            vertex_counts: vec![], 
            loaded: false, 
            material_count: 0,
            material_names: HashMap::new(),
            materials: HashMap::new(),
            lod_count: 0
        }
    }
}

/*
    Loading Functionality:
*/
#[derive(Resource)]
pub struct CornGLTFHandle(pub Option<Handle<Gltf>>);
impl Default for CornGLTFHandle{
    fn default() -> Self {
        Self(None)
    }
}

#[derive(Event)]
pub struct CornMeshLoadedEvent{}

#[derive(Event)]
pub struct CornGltfLoadedEvent{}

pub struct LoadCornPlugin<T> where T: States + Copy{
    active_state: T
}
impl<T> LoadCornPlugin<T> where T: States + Copy{
    pub fn new(active_state: T) -> Self {
        Self {active_state}
    }
}
impl<T> Plugin for LoadCornPlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .add_event::<CornMeshLoadedEvent>()
            .add_event::<CornGltfLoadedEvent>()
            .init_resource::<CornMeshes>()
            .init_resource::<CornGLTFHandle>()
            .add_systems(OnEnter(self.active_state), add_corn_load_task)
            .add_systems(Update, (
                save_corn_models.run_if(corn_model_loaded.and_then(run_once())),
                spawn_corn_combine_task.run_if(on_event::<CornGltfLoadedEvent>().and_then(run_once())),
                handle_combine_corn_tasks,
                remove_corn_load_task.run_if(on_event::<CornMeshLoadedEvent>().and_then(run_once()))
            ).run_if(in_state(self.active_state)));
        //Setup renderapp cornmeshes resource
        app.get_sub_app_mut(RenderApp).expect("RenderApp Doesnt Exist?")
            .init_resource::<CornMeshes>()
            .add_systems(ExtractSchedule, clone_corn_resource);
    }
}

fn add_corn_load_task(
    mut task_count: ResMut<LoadingTaskCount>,
    server: Res<AssetServer>,
    mut corn_handles: ResMut<CornGLTFHandle>,
    mut next_state: ResMut<NextState<CornLoadState>>
){
    task_count.0 += 1;
    corn_handles.0 = Some(server.load("models/corn_master.glb"));
    next_state.set(CornLoadState::Loading);
}
fn remove_corn_load_task(
    mut task_count: ResMut<LoadingTaskCount>
){
    task_count.0 -= 1;
}

fn corn_model_loaded(
    mut events: EventReader<AssetEvent<Gltf>>,
    corn_handle: Res<CornGLTFHandle>
) -> bool{
    events.read().any(|ev| match ev{
        AssetEvent::LoadedWithDependencies {id} => {*id == corn_handle.0.as_ref().unwrap().id()},
        _ => {false}
    })
}
fn save_corn_models(
    mut storage: ResMut<CornMeshes>,
    corn_gltf_handle: Res<CornGLTFHandle>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_mesh_assets: Res<Assets<GltfMesh>>,
    mut ev_writer: EventWriter<CornGltfLoadedEvent>,
){
    if let Some(handle) = &corn_gltf_handle.0{
        let gltf = gltf_assets.get(handle.id()).unwrap();
        let mut materials: Vec<Handle<StandardMaterial>> = vec![];
        let mut unsorted: Vec<(usize, Vec<(Handle<Mesh>, usize)>)> = gltf.named_meshes.iter().map(|(name, gmesh_handle)| {
            (
                name[7..].parse::<usize>().unwrap(),
                gltf_mesh_assets.get(gmesh_handle).unwrap().primitives.iter().map(|p| {
                    if let Some(index) = materials.iter().position(|a| a.eq(p.material.as_ref().unwrap())){
                        return (p.mesh.clone(), index);
                    }
                    materials.push(p.material.clone().unwrap());
                    return (p.mesh.clone(), materials.len()-1);
                }).collect::<Vec<(Handle<Mesh>, usize)>>()
            )
        }).collect();
        unsorted.sort_by(|a, b| a.0.cmp(&b.0));
        storage.lod_groups = unsorted.into_iter().map(|(_, b)| b).collect();
        storage.material_names = HashMap::<usize, String>::from_iter(gltf.named_materials.iter().filter_map(|(s, m)| {
            if let Some(index) = materials.iter().position(|val| *val==*m){
                return Some((index, s.to_owned()));
            }else{
                return None;
            }
        }));
        storage.materials = gltf.named_materials.clone();
        storage.lod_count = storage.lod_groups.len() as u32;
        storage.material_count = materials.len();
        ev_writer.send(CornGltfLoadedEvent{});
    }
}

#[derive(Component)]
pub struct CombineCornTask(Task<(Mesh, Vec<(usize, Vec<usize>, usize)>)>);

fn spawn_corn_combine_task(
    mut commands: Commands, 
    meshes: Res<Assets<Mesh>>, 
    corn: Res<CornMeshes>
){
    let threads = AsyncComputeTaskPool::get();
    let corn_meshes: Vec<Vec<(Mesh, usize)>> = corn.lod_groups
        .iter()
        .map(|vec| 
            vec.iter().map(|(h, i)| (meshes.get(h).unwrap().clone(), *i)).collect()
        ).collect();
    let task = threads.spawn(corn_combine_task(corn_meshes));
    commands.spawn(CombineCornTask(task));
}
async fn corn_combine_task(meshes: Vec<Vec<(Mesh, usize)>>) -> (Mesh, Vec<(usize, Vec<usize>, usize)>){
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetPersistencePolicy::Unload);
    let mut vertex_counts: Vec<(usize, Vec<usize>, usize)> = vec![];

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normal: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut tangent: Vec<[f32; 4]> = Vec::new();

    let mut materials: Vec<u32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut indices_offset: u32 = 0;
    for lod in meshes.iter(){
        vertex_counts.push((indices.len(), vec![], 0));
        for (mesh, mat) in lod.iter(){
            if let Some(Indices::U16(mesh_indices)) = mesh.indices(){
                indices.extend(mesh_indices.iter().map(|index| *index as u32+indices_offset));
                if let Some(VertexAttributeValues::Float32x3(vertex_positions)) = 
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    positions.extend(vertex_positions);
                    if let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL){
                        normal.extend(normals);
                    }
                    if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0){
                        uv.extend(uvs);
                    }
                    if let Some(VertexAttributeValues::Float32x4(tangents)) = mesh.attribute(Mesh::ATTRIBUTE_TANGENT){
                        tangent.extend(tangents);
                    }
                    materials.extend([*mat as u32].repeat(vertex_positions.len()));
                    indices_offset += vertex_positions.len() as u32;
                }
                vertex_counts.last_mut().map(|val| val.1.push(mesh_indices.len()));
            }
        }
        vertex_counts.last_mut().map(|val| {val.2 = indices.len()-val.0; val});
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    if normal.len() > 0{mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);}
    if uv.len() > 0{mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);}
    if tangent.len() > 0{mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangent);}
    mesh.insert_attribute(
        MeshVertexAttribute::new("Mesh_Index", 7, VertexFormat::Uint32), 
        materials
    );
    mesh.set_indices(Some(Indices::U32(indices)));
    
    (mesh, vertex_counts)
}
fn handle_combine_corn_tasks(
    mut tasks: Query<(Entity, &mut CombineCornTask)>,
    mut corn: ResMut<CornMeshes>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ev_writer: EventWriter<CornMeshLoadedEvent>,
    mut next_state: ResMut<NextState<CornLoadState>>
){
    for (entity, mut task) in &mut tasks {
        if let Some((global_mesh, vertex_count)) = future::block_on(future::poll_once(&mut task.0)) {
            let handle = meshes.add(global_mesh);
            corn.global_mesh = Some(handle);
            corn.vertex_counts = vertex_count;
            corn.loaded = true;
            commands.entity(entity).despawn();
            ev_writer.send(CornMeshLoadedEvent{});
            next_state.set(CornLoadState::Loaded);
        }
    }
}

fn clone_corn_resource(
    mut render_corn: ResMut<CornMeshes>, 
    main_corn: Extract<Res<CornMeshes>>,
){
    if main_corn.loaded && (!render_corn.loaded || main_corn.is_changed()){
        *render_corn = main_corn.clone();
    }
}
