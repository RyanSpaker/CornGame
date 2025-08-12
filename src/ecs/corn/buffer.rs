use std::sync::atomic::{AtomicBool, Ordering};
use bevy::{prelude::*, render::{render_resource::*, renderer::RenderDevice, Render, RenderApp, RenderSet}};
use bytemuck::{Pod, Zeroable};
use super::{asset::{CornLodInfo, CornModel}, CornField};

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

/// Component for Corn Fields containing the instance buffer
#[derive(Debug, Clone, Component)]
pub struct InstanceBuffer{
    pub buffer: Buffer, 
    pub instance_count: u64
}
impl InstanceBuffer{
    pub fn create_buffer(label: String, count: u64, render_device: &RenderDevice) -> Self{
        Self{
            buffer: render_device.create_buffer(&BufferDescriptor{
                label: Some(label.as_str()),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                size: count*CornData::DATA_SIZE,
                mapped_at_creation: false
            }), 
            instance_count: count
        }
    }
    pub fn create_buffer_with_data(label: String, render_device: &RenderDevice, data: &[u8]) -> Self{
        Self{
            buffer: render_device.create_buffer_with_data(&BufferInitDescriptor { 
                label: Some(label.as_str()), 
                contents: data, 
                usage:  BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            }), 
            instance_count: data.len() as u64/CornData::DATA_SIZE
        }
    }
}

/// Component for Corn Fields containing the vertex instance buffer
#[derive(Debug, Component)]
pub struct VertexInstanceBuffer{
    pub buffer: Buffer,
    pub instance_count: u64,
    /// Whether the buffer has data ready to be rendered written in it.
    pub ready: AtomicBool
}
impl VertexInstanceBuffer{
    pub fn create_buffer(label: String, count: u64, render_device: &RenderDevice) -> Self{
        Self{
            buffer: render_device.create_buffer(&BufferDescriptor{
                label: Some(label.as_str()),
                usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                size: count*CornData::VERTEX_DATA_SIZE,
                mapped_at_creation: false
            }), 
            instance_count: count,
            ready: AtomicBool::new(false)
        }
    }
    pub fn reset_ready_buffers(
        query: Query<&Self>
    ){
        for Self{ready, ..} in query.iter(){
            ready.store(false, Ordering::Relaxed);
        }
    }
}

/// Component for Corn Fields containing the indirect buffer
#[derive(Debug, Clone, Component)]
pub struct IndirectBuffer{
    pub buffer: Buffer,
    pub lod_count: usize
}
impl IndirectBuffer{
    /// System which updates per field indirect buffers as needed
    fn update_buffers(
        mut commands: Commands,
        fields: Query<(Entity, Option<&Self>, &CornLodInfo), (With<CornField>, With<CornModel>, Or<(Changed<CornLodInfo>, Without<Self>)>)>,
        render_device: Res<RenderDevice>
    ){
        let mut batch = vec![];
        for (entity, buffer, lod_info) in fields.iter(){
            // If buffer exists and is correctly sized, do nothing
            let lod_count = lod_info.0.len();
            if buffer.is_some_and(|b| b.lod_count == lod_count) {continue;}
            // Otherwise create a new buffer
            batch.push((entity, Self{
                buffer: render_device.create_buffer(&BufferDescriptor{
                    label: Some("Corn Field Indirect Buffer"),
                    size: lod_count as u64*20,
                    usage: BufferUsages::INDIRECT | BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                    mapped_at_creation: false
                }), 
                lod_count
            }));
        }
        commands.insert_batch(batch);
    }
}

#[derive(Default, Debug, Clone)]
pub struct CornBufferPlugin;
impl Plugin for CornBufferPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<CornData>()
        .sub_app_mut(RenderApp)
            .add_systems(Render, (
                IndirectBuffer::update_buffers.in_set(RenderSet::PrepareResources),
                VertexInstanceBuffer::reset_ready_buffers.in_set(RenderSet::Cleanup)
            ));
    }
}
