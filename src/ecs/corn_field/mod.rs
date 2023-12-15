pub mod scan_prepass;
pub mod render;
pub mod data_pipeline;
pub mod corn_fields;
//pub mod data_pipeline_old;

use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::RenderDevice, 
        RenderApp, RenderSet, Render,
    }
};
use bytemuck::{Pod, Zeroable};
use crate::prelude::corn_model::CornMeshes;
use self::{data_pipeline::{storage_manager::{CornBufferStorageManager, BufferRange}, MasterCornFieldDataPipelinePlugin}, render::CornRenderPlugin, scan_prepass::{CornBufferPrepassPlugin, VoteScanCompactBuffers}};

#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType)]
#[repr(C)]
pub struct PerCornData{
    offset: Vec3,
    scale: f32,
    rotation: Vec2,
    uuid: u32, //32 possible uuids, one per bit for use in a bitmask
    enabled: u32
}

pub const CORN_DATA_SIZE: u64 = 32;

/// This trait represents all implementation specific corn field settings
/// Impl this trait to create a type of corn field
/// Make sure to add a RenderableCornFieldPlugin<T> to the app as well
pub trait RenderableCornField: Component + Clone{
    /// This function returns a hash of the component used for an ID. 
    /// The hash value should use any values that change the structure of the corn, and none others.
    /// Any time the hash is changed, the corn is deleted off the gpu and re-initialized.
    /// Every unique corn field is expected to have a unique hash as well, so make sure the hashes use enough unique values
    fn gen_id(&self) -> RenderableCornFieldID;
    /// This function returns true when the corn field is ready for initialization, and all assets needed are loaded
    fn assets_ready(&self) -> bool {true}
    /// This function returns whether or not the corn field needs its elements to be continously positioned in the buffer
    fn needs_continuos_buffer_space() -> bool {false}
    /// Returns the total number of pieces of corn this field will create
    fn get_instance_count(&self) -> u64;
    /// Returns the corn fields bind group layout used in its init shader
    fn init_bind_group_descriptor<'a>() -> BindGroupLayoutDescriptor<'a>;
    /// Returns the push constant ranges used in the init shader
    fn init_push_constant_ranges() -> Vec<PushConstantRange> {vec![]}
    /// Returns the path to the init shader
    fn init_shader() -> String;
    /// Returns the shaderdefs used by the init shader
    fn init_shader_defs() -> Vec<ShaderDefVal> {vec![]}
    /// Returns the init entrypoint
    fn init_entry_point() -> String;
    /// This function is responsible for creating the necessary resources for initialization of the field data on the gpu
    /// If the field's init function can be batched with other fields of the same type, then it should return a single (BindGroup, u64)
    /// The Vec<Buffer> should return all buffers created for use in the init shaders
    fn get_init_resources(
        fields: Vec<(&Self, BufferRange, RenderableCornFieldID, String)>,
        render_device: &RenderDevice,
        layout: &BindGroupLayout,
        operation_buffer: &Buffer
    ) -> (Vec<(BindGroup, u64)>, Vec<Buffer>);
}

/// This component holds the hash id for a single corn field in a recognizable struct.
/// This makes it so that you can query all corn fields in the render app at once
#[derive(Debug, Clone, Hash, PartialEq, Eq, Component)]
pub struct RenderableCornFieldID{
    id: u64
}
impl From<u64> for RenderableCornFieldID{
    fn from(value: u64) -> Self {
        Self{id: value}
    }
}


#[derive(Resource, Default, Clone)]
pub struct CornInstanceBuffer{
    data_buffer: Option<Buffer>,
    sorted_buffer: Option<Buffer>,
    data_count: u64,
    data_ready: bool,
    indirect_buffer: Option<Buffer>,
    lod_count: u32,
    indirect_ready: bool,
    enabled: bool
}
impl CornInstanceBuffer{
    /// Runs during cleanup, making sure the resource has a correct indirect buffer when needed
    pub fn update_indirect_buffer(
        mut instance_buffer: ResMut<CornInstanceBuffer>,
        render_device: Res<RenderDevice>,
        corn_meshes: Res<CornMeshes>
    ){
        //Check if we need an indirect buffer
        if !corn_meshes.loaded || !instance_buffer.data_ready {return;}
        // check if we already have one
        if instance_buffer.indirect_ready && instance_buffer.lod_count == corn_meshes.lod_count {return;}
        // We need one, and its wrong, so we have to create it
        let data: Vec<u32> = (0..corn_meshes.lod_count).into_iter()
            .flat_map(|i| [
                corn_meshes.vertex_counts[i as usize].2 as u32, 
                0, 
                corn_meshes.vertex_counts[i as usize].0 as u32, 
                0, 
                0
            ].into_iter()).collect();
        // Replace indirect buffer
        instance_buffer.indirect_buffer.replace(render_device.create_buffer_with_data(&BufferInitDescriptor { 
            label: Some("Corn Indirect Buffer".into()), 
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT, 
            contents: bytemuck::cast_slice(&data[..]) 
        })).and_then(|old_buffer| Some(old_buffer.destroy()));
        instance_buffer.lod_count = corn_meshes.lod_count;
        instance_buffer.indirect_ready = true;
        instance_buffer.enabled = true;
    }
    /// Called by the operation executor, replaces the data buffer with a new one
    pub fn swap_instance_buffer(&mut self, new_data: Buffer, new_size: u64, render_device: &RenderDevice) {
        self.data_buffer.replace(new_data).and_then(|old_buffer| Some(old_buffer.destroy()));
        self.sorted_buffer.replace(render_device.create_buffer(&BufferDescriptor{
            label: Some("Corn Instance Index Buffer".into()),
            size: new_size*CORN_DATA_SIZE,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
            mapped_at_creation: false
        })).and_then(|old_buffer| Some(old_buffer.destroy()));
        self.data_count = new_size;
        self.data_ready = true;
        self.enabled = self.indirect_ready;
    }
    /// Returns the number of instance position in the buffer
    pub fn get_instance_count(&self) -> u64 {return self.data_count;}
    /// Returns the data buffer
    pub fn get_instance_buffer(&self) -> Option<&Buffer> {return self.data_buffer.as_ref()}
    /// Runs during cleanup, deletes the buffers if no corn is in them
    pub fn cleanup(
        mut instance_buffer: ResMut<CornInstanceBuffer>,
        storage: Res<CornBufferStorageManager>
    ){
        if storage.ranges.is_empty() && storage.stale_space.is_empty(){
            instance_buffer.data_buffer.take().and_then(|buffer| Some(buffer.destroy()));
            instance_buffer.sorted_buffer.take().and_then(|buffer| Some(buffer.destroy()));
            instance_buffer.indirect_buffer.take().and_then(|buffer| Some(buffer.destroy()));
            instance_buffer.data_count = 0;
            instance_buffer.data_ready = false;
            instance_buffer.lod_count = 0;
            instance_buffer.indirect_ready = false;
            instance_buffer.enabled = false;
        }
    }
    /// Returns whether or not the corn data is ready to begin rendering
    pub fn ready_to_render(&self) -> bool{
        return self.enabled;
    }
}

/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((MasterCornFieldDataPipelinePlugin{}, CornRenderPlugin{}, CornBufferPrepassPlugin{}))
        .sub_app_mut(RenderApp)
            .init_resource::<CornInstanceBuffer>()
            .add_systems(Render, (
                CornInstanceBuffer::cleanup.after(CornBufferStorageManager::handle_shrink_events),
                CornInstanceBuffer::update_indirect_buffer,
                VoteScanCompactBuffers::cleanup
            ).chain().in_set(RenderSet::Cleanup));
    }
}
