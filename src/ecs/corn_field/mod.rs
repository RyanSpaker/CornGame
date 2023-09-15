pub mod init;
pub mod render;

use std::mem::size_of;
use bevy::{
    prelude::*,
    render::{
        extract_component::ExtractComponent,
        render_resource::*,
    }, utils::hashbrown::HashMap
};
use bytemuck::{Pod, Zeroable};
use self::init::{CornFieldDataState, CornFieldInitPlugin};

/// Per Instance Corn Data
/// contains the offset from the position of the corn field entity
/// the scale is tacked onto the end of the offset_scale vector
/// the rotation takes up the first 2 values of the rotation vector (sin and cos of the angle)
/// the struct has to be a multiple of 16 bytes long due to storage buffer limitations
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C)]
pub struct PerCornData{
    offset_scale: Vec4,
    rotation: Vec4
}

/// Component that stores the instance data of a corn field
/// Added to corn field entities after the gpu initializes the data
/// only if RenderedCornFields.read_back_data is true
#[derive(Component)]
pub struct CornInstanceData{
    data: Vec<PerCornData>
}

/// The component representing a corn field
/// center is the position of the corn field's center relative to the entities world position
/// half_extents is the width and length of the corn field in world units
/// resolution is the number of corn stalks along the width and length
/// height_range is the min-max range of height scalars to randomize the corn with
#[derive(Component, ExtractComponent, Clone, Debug)]
pub struct CornField{
    center: Vec3,
    half_extents: Vec2,
    resolution: (usize, usize),
    height_range: Vec2
}
impl CornField{
    //creates instance data from width and density values
    pub fn new(center: Vec3, half_extents: Vec2, resolution: (usize, usize), height_range: Vec2) -> Self{
        Self{
            center, 
            half_extents, 
            resolution,
            height_range
        }
    }
}

/// Resource to store all of the corn fields and their data in the RenderApp
#[derive(Resource)]
pub struct RenderedCornFields{
    fields: HashMap<Entity, CornFieldRenderData>,
    read_back_data: bool
}
impl Default for RenderedCornFields{
    fn default() -> Self {
        Self{fields: HashMap::new(), read_back_data: false}
    }
}

/// Stores the renderapp data for a single corn field.
/// This includes corn field settings present on a corn field component,
/// the buffers used to store the data, and its current state
/// in the loading process
pub struct CornFieldRenderData{
    state: CornFieldDataState,
    center: Vec3,
    half_extents: Vec2,
    resolution: (usize, usize),
    height_range: Vec2,
    data: Option<Vec<PerCornData>>,
    instance_buffer: Option<Buffer>,
    instance_buffer_bind_group: Option<BindGroup>,
    cpu_readback_buffer: Option<Buffer>
}
impl CornFieldRenderData{
    pub fn get_byte_count(&self) -> u64{
        return (self.resolution.0*self.resolution.1*size_of::<PerCornData>()) as u64;
    }
}

/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CornFieldInitPlugin);
    }
}
