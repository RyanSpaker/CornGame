pub mod data_pipeline;
pub mod scan_prepass;
pub mod render;

use std::mem::size_of;
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::RenderDevice, 
        RenderApp, extract_component::ExtractComponent, RenderSet, Render,
    }
};
use bytemuck::{Pod, Zeroable};
use crate::prelude::corn_model::{CornMeshes, CornLodCount};
use self::{data_pipeline::CornFieldDataPipelinePlugin, scan_prepass::CornBufferPrepassPlugin, render::CornRenderPlugin};

#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType)]
#[repr(C)]
pub struct PerCornData{
    offset: Vec3,
    scale: f32,
    rotation: Vec2,
    uuid: u32, //32 possible uuids, one per bit for use in a bitmask
    enabled: u32
}

#[derive(Component, ExtractComponent, Clone, Debug)]
pub struct CornField{
    center: Vec3,
    half_extents: Vec2,
    resolution: (u32, u32),
    height_range: Vec2
}
impl CornField{
    //creates instance data from width and density values
    pub fn new(center: Vec3, half_extents: Vec2, resolution: (u32, u32), height_range: Vec2) -> Self{
        Self{
            center, 
            half_extents, 
            resolution,
            height_range
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct CornInstanceBuffer{
    data_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    indirect_buffer: Option<Buffer>,
    data_count: u32,
    lod_count: u32
}
impl CornInstanceBuffer{
    pub fn initialize_data(&mut self, render_device: &RenderDevice, init_size: u64) -> bool{
        if self.data_buffer.is_none() {
            self.data_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Corn Instance Buffer"), 
                size: init_size * size_of::<PerCornData>() as u64, 
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
                mapped_at_creation: false
            }));
            self.index_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Corn Instance Index Buffer"), 
                size: init_size * size_of::<PerCornData>() as u64, 
                usage: BufferUsages::STORAGE | BufferUsages::VERTEX, 
                mapped_at_creation: false
            }));
            self.data_count = init_size as u32;
        }
        return true;
    }
    pub fn update_indirect(&mut self, render_device: &RenderDevice, lod_count: u32, meshes: &CornMeshes){
        let mut data = vec![vec![0u32; 5]; lod_count as usize];
        for i in 0..lod_count{
            data[i as usize][0] = meshes.vertex_counts[i as usize].2 as u32;
            data[i as usize][2] = meshes.vertex_counts[i as usize].0 as u32;
        }
        let data: Vec<u32> = data.into_iter().flat_map(|vec| vec.into_iter()).collect();
        if self.indirect_buffer.is_none() || self.lod_count != lod_count{
            self.indirect_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
                label: Some("Corn Indirect Buffer"), 
                usage: BufferUsages::STORAGE | BufferUsages::INDIRECT,
                contents: bytemuck::cast_slice(data.as_slice())
            }));
            self.lod_count = lod_count
        }
    }
    pub fn get_instance_count(&self) -> u32 {return self.data_count;}
    pub fn get_instance_buffer(&self) -> Option<&Buffer> {return self.data_buffer.as_ref()}
    pub fn swap_data_buffers(&mut self, new_data: &Buffer, new_size: u32, render_device: &RenderDevice){
        if let Some(buffer) = self.data_buffer.as_mut(){
            buffer.destroy();
        }
        if let Some(buffer) = self.index_buffer.as_mut(){
            buffer.destroy();
        }
        self.data_buffer = Some(new_data.to_owned());
        self.index_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Index Buffer"), 
            size: new_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX, 
            mapped_at_creation: false
        }));
        self.data_count = new_size;
    }
    pub fn destroy(&mut self){
        if let Some(buffer) = self.data_buffer.as_ref(){buffer.destroy(); self.data_buffer = None;}
        if let Some(buffer) = self.index_buffer.as_ref(){buffer.destroy(); self.index_buffer = None;}
        self.data_count = 0;
    }
    pub fn ready_to_render(&self) -> bool{
        return self.data_buffer.is_some() && self.index_buffer.is_some() && self.indirect_buffer.is_some();
    }
}

/// Makes sure the instance buffer's indirect buffer has the correct size
pub fn update_indirect_buffer(
    mut instance_buffer: ResMut<CornInstanceBuffer>,
    render_device: Res<RenderDevice>,
    meshes: Res<CornMeshes>,
    lod_count: Res<CornLodCount>
){
    if !meshes.loaded || instance_buffer.data_count == 0{return;}
    instance_buffer.update_indirect(&render_device, lod_count.0, &meshes);
}

/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((CornFieldDataPipelinePlugin, CornBufferPrepassPlugin{}, CornRenderPlugin{}))
        .sub_app_mut(RenderApp)
            .init_resource::<CornInstanceBuffer>()
            .add_systems(Render, update_indirect_buffer.in_set(RenderSet::Cleanup));
    }
}
