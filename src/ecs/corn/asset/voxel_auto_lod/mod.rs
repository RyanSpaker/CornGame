use std::collections::VecDeque;
use bevy::{prelude::*, render::{mesh::{Indices, VertexAttributeValues}, render_asset::RenderAssetUsages}, tasks::{AsyncComputeTaskPool, Task}, utils::hashbrown::HashSet};
use futures_lite::future::{block_on, poll_once};
use super::{CornAsset, CornModel};

pub struct AutoLodPlugin;
impl Plugin for AutoLodPlugin{
    fn build(&self, app: &mut App){
        app.add_systems(Update, (spawn_voxel_task.run_if(on_event::<AssetEvent<CornAsset>>()), spawn_mesh));
    }
}

fn spawn_voxel_task(mut events: EventReader<AssetEvent<CornAsset>>, corn: Res<CornModel>, asset: Res<Assets<CornAsset>>, meshes: Res<Assets<Mesh>>, mut commands: Commands){
    if events.read().any(|event| event.is_loaded_with_dependencies(corn.asset.clone())){
        let corn_asset = asset.get(corn.asset.clone()).unwrap();
        let mesh_handle = corn_asset.master_mesh.clone();
        let mesh = meshes.get(mesh_handle).unwrap().clone();
        let task_pool = AsyncComputeTaskPool::get();
        let task: Task<Mesh> = task_pool.spawn(voxel_auto_lod(mesh, corn_asset.lod_data[0].total_vertices, 1.0));
        commands.spawn(VoxelMeshTask(task));
    }
}

fn spawn_mesh(mut meshes: ResMut<Assets<Mesh>>, mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>, mut query: Query<(Entity, &mut VoxelMeshTask)>){
    for (entity, mut task) in &mut query {
        if let Some(mesh) = block_on(poll_once(&mut task.0)) {
            commands.spawn(MaterialMeshBundle{
                mesh: meshes.add(mesh),
                material: materials.add(StandardMaterial::from(Color::WHITE)),
                ..default()
            });
            commands.get_entity(entity).unwrap().despawn();
        }
    }
}


#[derive(Debug, Component)]
pub struct VoxelMeshTask(Task<Mesh>);

/// Async task for testing auto_lod with voxelization algorithm
async fn voxel_auto_lod(mesh: Mesh, indice_limit: usize, voxel_size: f32) -> Mesh{
    println!("Hey!");
    let indices: Vec<u32> = match mesh.indices().unwrap(){
        Indices::U16(data) => data.iter().map(|val| *val as u32).take(indice_limit).collect(),
        Indices::U32(data) => data.iter().map(|val| *val).take(indice_limit).collect()
    };
    let vertices = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec();
    let triangles: Vec<Triangle> = indices.chunks_exact(3).map(|tri| {
        Triangle::new(vertices[tri[0] as usize], vertices[tri[1] as usize], vertices[tri[2] as usize])
    }).collect();
    let origin = Vec3::new(-2.0, -1.0, -2.0);
    let step = Vec3::ONE*voxel_size;
    let istep = step.recip();
    let resolution = ((4.0 / voxel_size).ceil() as usize, (4.0 / voxel_size).ceil() as usize, (4.0 / voxel_size).ceil() as usize);
    // x, y, z
    let mut voxels: Vec<Vec<Vec<usize>>> = vec![vec![vec![0; resolution.2]; resolution.1]; resolution.0];
    let mut face_id: Vec<(usize, usize, usize, usize, usize, usize)> = vec![(0, 0, 0, 0, 0, 0)];
    let mut voxel_queue: VecDeque<(usize, usize, usize)> = VecDeque::new();
    let mut voxel_count = 0;
    println!("Starting Voxelization");
    for triangle in triangles.iter(){
        let start = ((triangle.min - origin) * istep).as_uvec3();
        let end = ((triangle.max - origin) * istep).as_uvec3() + UVec3::ONE;
        for x in start.x..end.x{
            for y in start.y..end.y{
                for z in start.z..end.z{
                    let cur_pos = origin + step*Vec3::new(x as f32, y as f32, z as f32);
                    let aabb = Aabb::new(cur_pos, cur_pos + step);
                    if voxels[x as usize][y as usize][z as usize] == 0 && intersects(triangle.clone(), aabb) {
                        voxels[x as usize][y as usize][z as usize] = face_id.len();
                        face_id.push((0, 0, 0, 0, 0, 0)); 
                        voxel_count += 1;
                        voxel_queue.push_back((x as usize, y as usize, z as usize));
                    }
                }
            }
        }
    }
    println!("Total Voxels: {}", voxel_count);
    //construct voxel mesh
    let mut free_id: usize = 2;
    let mut groups: Vec<(FaceDir, VecDeque<(usize, usize, usize)>)> = vec![];
    for (x, y, z) in voxel_queue.drain(..){
        // Check if we have a top faced voxel without a group assigned to it
        if voxels[x][y+1][z] == 0 {
            if face_id[voxels[x][y][z]].0 < 2{
                // create a new group
                let cur_id = free_id;
                free_id += 1;
                let mut group_indices: VecDeque<(usize, usize, usize)> = VecDeque::new();
                let mut bleed_deq: VecDeque<(usize, usize, usize)> = VecDeque::new();
                bleed_deq.push_back((x, y, z)); group_indices.push_back((x, y, z));
                while !bleed_deq.is_empty(){
                    let (i, j, k) = bleed_deq.pop_front().unwrap();
                    face_id[voxels[i][j][k]].0 = cur_id;
                    if voxels[i+1][j][k] != 0 && face_id[voxels[i+1][j][k]].0 == 0 && voxels[i+1][j+1][k] == 0 {bleed_deq.push_back((i+1, j, k)); group_indices.push_back((i+1, j, k));}
                    if voxels[i-1][j][k] != 0 && face_id[voxels[i-1][j][k]].0 == 0 && voxels[i-1][j+1][k] == 0 {bleed_deq.push_back((i-1, j, k)); group_indices.push_back((i-1, j, k));}
                    if voxels[i][j][k+1] != 0 && face_id[voxels[i][j][k+1]].0 == 0 && voxels[i][j+1][k+1] == 0 {bleed_deq.push_back((i, j, k+1)); group_indices.push_back((i, j, k+1));}
                    if voxels[i][j][k-1] != 0 && face_id[voxels[i][j][k-1]].0 == 0 && voxels[i][j+1][k-1] == 0 {bleed_deq.push_back((i, j, k-1)); group_indices.push_back((i, j, k-1));}
                }
                groups.push((FaceDir::Top, group_indices));
            }
        }else{face_id[voxels[x][y][z]].0 = 1;}
        
        if voxels[x][y-1][z] == 0 {
            if face_id[voxels[x][y][z]].1 < 2{
                // create a new group
                let cur_id = free_id;
                free_id += 1;
                let mut group_indices: VecDeque<(usize, usize, usize)> = VecDeque::new();
                let mut bleed_deq: VecDeque<(usize, usize, usize)> = VecDeque::new();
                bleed_deq.push_back((x, y, z)); group_indices.push_back((x, y, z));
                while !bleed_deq.is_empty(){
                    let (i, j, k) = bleed_deq.pop_front().unwrap();
                    face_id[voxels[i][j][k]].1 = cur_id;
                    if voxels[i+1][j][k] != 0 && face_id[voxels[i+1][j][k]].1 == 0 && voxels[i+1][j-1][k] == 0 {bleed_deq.push_back((i+1, j, k)); group_indices.push_back((i+1, j, k));}
                    if voxels[i-1][j][k] != 0 && face_id[voxels[i-1][j][k]].1 == 0 && voxels[i-1][j-1][k] == 0 {bleed_deq.push_back((i-1, j, k)); group_indices.push_back((i-1, j, k));}
                    if voxels[i][j][k+1] != 0 && face_id[voxels[i][j][k+1]].1 == 0 && voxels[i][j-1][k+1] == 0 {bleed_deq.push_back((i, j, k+1)); group_indices.push_back((i, j, k+1));}
                    if voxels[i][j][k-1] != 0 && face_id[voxels[i][j][k-1]].1 == 0 && voxels[i][j-1][k-1] == 0 {bleed_deq.push_back((i, j, k-1)); group_indices.push_back((i, j, k-1));}
                }
                groups.push((FaceDir::Bottom, group_indices));
            }
        }else{face_id[voxels[x][y][z]].1 = 1;}
        
        if voxels[x-1][y][z] == 0 {
            if face_id[voxels[x][y][z]].2 < 2{
                // create a new group
                let cur_id = free_id;
                free_id += 1;
                let mut group_indices: VecDeque<(usize, usize, usize)> = VecDeque::new();
                let mut bleed_deq: VecDeque<(usize, usize, usize)> = VecDeque::new();
                bleed_deq.push_back((x, y, z)); group_indices.push_back((x, y, z));
                while !bleed_deq.is_empty(){
                    let (i, j, k) = bleed_deq.pop_front().unwrap();
                    face_id[voxels[i][j][k]].2 = cur_id;
                    if voxels[i][j+1][k] != 0 && face_id[voxels[i][j+1][k]].2 == 0 && voxels[i-1][j+1][k] == 0 {bleed_deq.push_back((i, j+1, k)); group_indices.push_back((i, j+1, k));}
                    if voxels[i][j-1][k] != 0 && face_id[voxels[i][j-1][k]].2 == 0 && voxels[i-1][j-1][k] == 0 {bleed_deq.push_back((i, j-1, k)); group_indices.push_back((i, j-1, k));}
                    if voxels[i][j][k+1] != 0 && face_id[voxels[i][j][k+1]].2 == 0 && voxels[i-1][j][k+1] == 0 {bleed_deq.push_back((i, j, k+1)); group_indices.push_back((i, j, k+1));}
                    if voxels[i][j][k-1] != 0 && face_id[voxels[i][j][k-1]].2 == 0 && voxels[i-1][j][k-1] == 0 {bleed_deq.push_back((i, j, k-1)); group_indices.push_back((i, j, k-1));}
                }
                groups.push((FaceDir::Left, group_indices));
            }
        }else{face_id[voxels[x][y][z]].2 = 1;}

        if voxels[x+1][y][z] == 0 {
            if face_id[voxels[x][y][z]].3 < 2{
                // create a new group
                let cur_id = free_id;
                free_id += 1;
                let mut group_indices: VecDeque<(usize, usize, usize)> = VecDeque::new();
                let mut bleed_deq: VecDeque<(usize, usize, usize)> = VecDeque::new();
                bleed_deq.push_back((x, y, z)); group_indices.push_back((x, y, z));
                while !bleed_deq.is_empty(){
                    let (i, j, k) = bleed_deq.pop_front().unwrap();
                    face_id[voxels[i][j][k]].3 = cur_id;
                    if voxels[i][j+1][k] != 0 && face_id[voxels[i][j+1][k]].3 == 0 && voxels[i+1][j+1][k] == 0 {bleed_deq.push_back((i, j+1, k)); group_indices.push_back((i, j+1, k));}
                    if voxels[i][j-1][k] != 0 && face_id[voxels[i][j-1][k]].3 == 0 && voxels[i+1][j-1][k] == 0 {bleed_deq.push_back((i, j-1, k)); group_indices.push_back((i, j-1, k));}
                    if voxels[i][j][k+1] != 0 && face_id[voxels[i][j][k+1]].3 == 0 && voxels[i+1][j][k+1] == 0 {bleed_deq.push_back((i, j, k+1)); group_indices.push_back((i, j, k+1));}
                    if voxels[i][j][k-1] != 0 && face_id[voxels[i][j][k-1]].3 == 0 && voxels[i+1][j][k-1] == 0 {bleed_deq.push_back((i, j, k-1)); group_indices.push_back((i, j, k-1));}
                }
                groups.push((FaceDir::Right, group_indices));
            }
        }else{face_id[voxels[x][y][z]].3 = 1;}

        if voxels[x][y][z-1] == 0 {
            if face_id[voxels[x][y][z]].4 < 2{
                // create a new group
                let cur_id = free_id;
                free_id += 1;
                let mut group_indices: VecDeque<(usize, usize, usize)> = VecDeque::new();
                let mut bleed_deq: VecDeque<(usize, usize, usize)> = VecDeque::new();
                bleed_deq.push_back((x, y, z)); group_indices.push_back((x, y, z));
                while !bleed_deq.is_empty(){
                    let (i, j, k) = bleed_deq.pop_front().unwrap();
                    face_id[voxels[i][j][k]].4 = cur_id;
                    if voxels[i+1][j][k] != 0 && face_id[voxels[i+1][j][k]].4 == 0 && voxels[i+1][j][k-1] == 0 {bleed_deq.push_back((i+1, j, k)); group_indices.push_back((i+1, j, k));}
                    if voxels[i-1][j][k] != 0 && face_id[voxels[i-1][j][k]].4 == 0 && voxels[i-1][j][k-1] == 0 {bleed_deq.push_back((i-1, j, k)); group_indices.push_back((i-1, j, k));}
                    if voxels[i][j+1][k] != 0 && face_id[voxels[i][j+1][k]].4 == 0 && voxels[i][j+1][k-1] == 0 {bleed_deq.push_back((i, j+1, k)); group_indices.push_back((i, j+1, k));}
                    if voxels[i][j-1][k] != 0 && face_id[voxels[i][j-1][k]].4 == 0 && voxels[i][j-1][k-1] == 0 {bleed_deq.push_back((i, j-1, k)); group_indices.push_back((i, j-1, k));}
                }
                groups.push((FaceDir::Front, group_indices));
            }
        }else{face_id[voxels[x][y][z]].4 = 1;}

        if voxels[x][y][z+1] == 0 {
            if face_id[voxels[x][y][z]].5 < 2{
                // create a new group
                let cur_id = free_id;
                free_id += 1;
                let mut group_indices: VecDeque<(usize, usize, usize)> = VecDeque::new();
                let mut bleed_deq: VecDeque<(usize, usize, usize)> = VecDeque::new();
                bleed_deq.push_back((x, y, z)); group_indices.push_back((x, y, z));
                while !bleed_deq.is_empty(){
                    let (i, j, k) = bleed_deq.pop_front().unwrap();
                    face_id[voxels[i][j][k]].5 = cur_id;
                    if voxels[i+1][j][k] != 0 && face_id[voxels[i+1][j][k]].5 == 0 && voxels[i+1][j][k+1] == 0 {bleed_deq.push_back((i+1, j, k)); group_indices.push_back((i+1, j, k));}
                    if voxels[i-1][j][k] != 0 && face_id[voxels[i-1][j][k]].5 == 0 && voxels[i-1][j][k+1] == 0 {bleed_deq.push_back((i-1, j, k)); group_indices.push_back((i-1, j, k));}
                    if voxels[i][j+1][k] != 0 && face_id[voxels[i][j+1][k]].5 == 0 && voxels[i][j+1][k+1] == 0 {bleed_deq.push_back((i, j+1, k)); group_indices.push_back((i, j+1, k));}
                    if voxels[i][j-1][k] != 0 && face_id[voxels[i][j-1][k]].5 == 0 && voxels[i][j-1][k+1] == 0 {bleed_deq.push_back((i, j-1, k)); group_indices.push_back((i, j-1, k));}
                }
                groups.push((FaceDir::Back, group_indices));
            }
        }else{face_id[voxels[x][y][z]].5 = 1;}
    }

    println!("Group Count: {}", groups.len());

    let mut vertex_indices: Vec<Vec<Vec<Option<usize>>>> = vec![vec![vec![None; resolution.2+1]; resolution.1+1]; resolution.0+1];
    let mut vertices: Vec<Vec3> = vec![];
    let mut faces: Vec<[usize; 4]> = vec![];

    for (dir, group) in groups.into_iter(){
        let mut available_voxels: HashSet<(usize, usize, usize)> = HashSet::from_iter(group.into_iter());
        while !available_voxels.is_empty(){
            //find the largest square
            let mut largest_area = 0;
            let mut largest_area_dims: (usize, usize, usize) = (0, 0, 0);
            let mut largest_area_origin: (usize, usize, usize) = (0, 0, 0);
            for element in available_voxels.iter(){
                let mut pointer = element.to_owned();
                while available_voxels.contains(&pointer){
                    while available_voxels.contains(&pointer){
                        match dir{
                            FaceDir::Top => {pointer.2 += 1},
                            FaceDir::Bottom => {pointer.2 += 1},
                            FaceDir::Left => {pointer.2 += 1},
                            FaceDir::Right => {pointer.2 += 1},
                            FaceDir::Back => {pointer.0 += 1},
                            FaceDir::Front => {pointer.0 += 1}
                        }
                    }
                    match dir{
                        FaceDir::Top => {pointer.2 -= 1;},
                        FaceDir::Bottom => {pointer.2 -= 1;},
                        FaceDir::Left => {pointer.2 -= 1;},
                        FaceDir::Right => {pointer.2 -= 1;},
                        FaceDir::Back => {pointer.0 -= 1;},
                        FaceDir::Front => {pointer.0 -= 1;}
                    }
                    if (pointer.2 - element.2 + 1)*(pointer.0 - element.0 + 1)*(pointer.1 - element.1 + 1) > largest_area{
                        largest_area = (pointer.2 - element.2 + 1)*(pointer.0 - element.0 + 1)*(pointer.1 - element.1 + 1);
                        largest_area_dims = (pointer.0-element.0 + 1, pointer.1 - element.1 + 1, pointer.2 - element.2 + 1);
                        largest_area_origin = element.to_owned();
                    }
                    match dir{
                        FaceDir::Top => {pointer.2 = element.2; pointer.0 += 1;},
                        FaceDir::Bottom => {pointer.2 = element.2; pointer.0 += 1;},
                        FaceDir::Left => {pointer.2 = element.2; pointer.1 += 1;},
                        FaceDir::Right => {pointer.2 = element.2; pointer.1 += 1;},
                        FaceDir::Back => {pointer.0 = element.0; pointer.1 += 1;},
                        FaceDir::Front => {pointer.0 = element.0; pointer.1 += 1}
                    }
                }
            }
            let a = match dir{
                FaceDir::Front => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1+1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1+1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, largest_area_origin.1 as f32+1.0, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Right => if let Some(index) = vertex_indices[largest_area_origin.0+1][largest_area_origin.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0+1][largest_area_origin.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32+1.0, largest_area_origin.1 as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Back => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2+1] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2+1] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, largest_area_origin.1 as f32, largest_area_origin.2 as f32+1.0)*step);
                    vertices.len()-1
                },
                _ => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, largest_area_origin.1 as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                }
            };
            let b = match dir{
                FaceDir::Top => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1+1][largest_area_origin.2 + largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1+1][largest_area_origin.2 + largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, largest_area_origin.1 as f32+1.0, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Bottom => if let Some(index) = vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0 + largest_area_dims.0) as f32, largest_area_origin.1 as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Left => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, largest_area_origin.1 as f32, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Right => if let Some(index) = vertex_indices[largest_area_origin.0+1][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0+1][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32+1.0, (largest_area_origin.1 + largest_area_dims.1) as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Front => if let Some(index) = vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0 + largest_area_dims.0) as f32, largest_area_origin.1 as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Back => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2+1] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2+1] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, (largest_area_origin.1 + largest_area_dims.1) as f32, largest_area_origin.2 as f32+1.0)*step);
                    vertices.len()-1
                }
            };
            let c = match dir{
                FaceDir::Top => if let Some(index) = vertex_indices[largest_area_origin.0+largest_area_dims.0][largest_area_origin.1+1][largest_area_origin.2+largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0+largest_area_dims.0][largest_area_origin.1+1][largest_area_origin.2 + largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0+largest_area_dims.0) as f32, largest_area_origin.1 as f32 + 1.0, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Bottom => if let Some(index) = vertex_indices[largest_area_origin.0+largest_area_dims.0][largest_area_origin.1][largest_area_origin.2+largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0+largest_area_dims.0][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0+largest_area_dims.0) as f32, largest_area_origin.1 as f32, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Left => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2+largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2+largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, (largest_area_origin.1+largest_area_dims.1) as f32, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Right => if let Some(index) = vertex_indices[largest_area_origin.0+1][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2+largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0+1][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2+largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32 + 1.0, (largest_area_origin.1+largest_area_dims.1) as f32, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Front => if let Some(index) = vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0 + largest_area_dims.0) as f32, (largest_area_origin.1+largest_area_dims.1) as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Back => if let Some(index) = vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2+1] {index} else {
                    vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1+largest_area_dims.1][largest_area_origin.2+1] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0 + largest_area_dims.0) as f32, (largest_area_origin.1+largest_area_dims.1) as f32, largest_area_origin.2 as f32 + 1.0)*step);
                    vertices.len()-1
                }
            };
            let d = match dir{
                FaceDir::Bottom => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, largest_area_origin.1 as f32, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Top => if let Some(index) = vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1+1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1+1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0 + largest_area_dims.0) as f32, largest_area_origin.1 as f32 + 1.0, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Right => if let Some(index) = vertex_indices[largest_area_origin.0+1][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] {index} else {
                    vertex_indices[largest_area_origin.0+1][largest_area_origin.1][largest_area_origin.2 + largest_area_dims.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32 + 1.0, largest_area_origin.1 as f32, (largest_area_origin.2 + largest_area_dims.2) as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Left => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, (largest_area_origin.1 + largest_area_dims.1) as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                },
                FaceDir::Back => if let Some(index) = vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1][largest_area_origin.2+1] {index} else {
                    vertex_indices[largest_area_origin.0 + largest_area_dims.0][largest_area_origin.1][largest_area_origin.2+1] = Some(vertices.len());
                    vertices.push(origin + Vec3::new((largest_area_origin.0 + largest_area_dims.0) as f32, largest_area_origin.1 as f32, largest_area_origin.2 as f32 + 1.0)*step);
                    vertices.len()-1
                },
                FaceDir::Front => if let Some(index) = vertex_indices[largest_area_origin.0][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2] {index} else {
                    vertex_indices[largest_area_origin.0][largest_area_origin.1 + largest_area_dims.1][largest_area_origin.2] = Some(vertices.len());
                    vertices.push(origin + Vec3::new(largest_area_origin.0 as f32, (largest_area_origin.1 + largest_area_dims.1) as f32, largest_area_origin.2 as f32)*step);
                    vertices.len()-1
                }
            };
            faces.push([a, b, c, d]);
            for x in 0..largest_area_dims.0{
                for y in 0..largest_area_dims.1{
                    for z in 0..largest_area_dims.2{
                        available_voxels.remove(&(largest_area_origin.0 + x, largest_area_origin.1 + y, largest_area_origin.2 + z));
                    }
                }
            }
        }
    }

    println!("Face Count: {}", faces.len());

    let indices = Indices::U32(faces.into_iter().flat_map(|face| [face[0] as u32, face[1] as u32, face[2] as u32, face[2] as u32, face[3] as u32, face[0] as u32].into_iter()).collect::<Vec<u32>>());
    let vertices = VertexAttributeValues::from(vertices);

    let mut mesh = Mesh::new(wgpu_types::PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_indices(indices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    return mesh;
}

pub enum FaceDir{
    Top,
    Bottom,
    Left,
    Right,
    Front,
    Back
}

#[derive(Debug, Clone)]
pub struct Triangle{
    pub points: [Vec3; 3],
    pub min: Vec3,
    pub max: Vec3
}
impl Triangle{
    pub fn new(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Self{
        Self{
            points: [a.into(), b.into(), c.into()],
            min: Vec3::new(
                a[0].min(b[0]).min(c[0]),
                a[1].min(b[1]).min(c[1]),
                a[2].min(b[2]).min(c[2]),
            ),
            max: Vec3::new(
                a[0].max(b[0]).max(c[0]),
                a[1].max(b[1]).max(c[1]),
                a[2].max(b[2]).max(c[2]),
            )
        }
    }
    pub fn get_edges(&self) -> (Vec3, Vec3, Vec3){
        (self.points[1] - self.points[0], self.points[2] - self.points[1], self.points[0] - self.points[2])
    }
}
impl std::ops::SubAssign<Vec3> for Triangle{
    fn sub_assign(&mut self, rhs: Vec3) {
        self.points[0] -= rhs;
        self.points[1] -= rhs;
        self.points[2] -= rhs;
    }
}

#[derive(Debug, Clone)]
pub struct Aabb{
    pub center: Vec3,
    pub half_extents: Vec3
}
impl Aabb{
    pub fn new(min: Vec3, max: Vec3) -> Self{
        Self{center: (min+max)*0.5, half_extents: max - (min+max)*0.5}
    }
}

pub fn intersects(mut triangle: Triangle, aabb: Aabb) -> bool{
    triangle -= aabb.center;
    let (edge_a, edge_b, edge_c) = triangle.get_edges();
    let u0 = Vec3::X; let u1 = Vec3::Y; let u2 = Vec3::Z;
    let u0_a = u0.cross(edge_a); let u0_b = u0.cross(edge_b); let u0_c = u0.cross(edge_c);
    let u1_a = u1.cross(edge_a); let u1_b = u1.cross(edge_b); let u1_c = u1.cross(edge_c);
    let u2_a = u2.cross(edge_a); let u2_b = u2.cross(edge_b); let u2_c = u2.cross(edge_c);

    if !test_axis(u0_a, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u0_b, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u0_c, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u1_a, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u1_b, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u1_c, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u2_a, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u2_b, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u2_c, &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};

    if !test_axis(u0.clone(), &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u1.clone(), &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    if !test_axis(u2.clone(), &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};

    if !test_axis(edge_a.cross(edge_b), &triangle, &aabb.half_extents, &u0, &u1, &u2){return false};
    return true;
}
pub fn test_axis(axis: Vec3, triangle: &Triangle, half_extents: &Vec3, u0: &Vec3, u1: &Vec3, u2: &Vec3) -> bool {
    let p0 = triangle.points[0].dot(axis);
    let p1 = triangle.points[1].dot(axis);
    let p2 = triangle.points[2].dot(axis);
    let r = half_extents.dot(Vec3::new(u0.dot(axis), u1.dot(axis), u2.dot(axis)).abs());
    p0.min(p1).min(p2).max(-(p0.max(p1).max(p2))) <= r
}