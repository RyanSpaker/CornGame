pub mod init;

use std::{ops::Range, mem::size_of};
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderContext}, 
        extract_component::ExtractComponent,
    }, utils::hashbrown::HashMap, math::bool
};
use bytemuck::{Pod, Zeroable};
use self::init::*;

#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType)]
#[repr(C)]
pub struct PerCornData{
    offset: Vec3,
    scale: f32,
    rotation: Vec2,
    uuid: u32, //32 possible uuids, one per bit for use in a bitmask
    empty: u32
}

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

#[derive(Resource, Default)]
pub struct RenderedCornFields{
    pub buffer: DynamicInstanceBuffer,
    pub data: HashMap<Entity, CornFieldRenderData>,
    pub settings: CornInitShaderData
}
impl RenderedCornFields{
    pub fn init_buffer(&mut self, render_device: &RenderDevice) {
        if self.buffer.is_initialized(){return;}
        self.buffer.init(
            render_device, 
            "Corn Instance Buffer", 
            1000000, 
            size_of::<PerCornData>(), 
            BufferUsages::STORAGE | BufferUsages::VERTEX
        );
    }
    pub fn add_data(&mut self, entity: Entity, corn_field: &CornField){
        let new_data = CornFieldRenderData::new(
            corn_field.center, 
            corn_field.half_extents, 
            corn_field.resolution, 
            corn_field.height_range
        );
        if let Some(old_data) = self.data.insert(entity, new_data){
            self.buffer.add_ranges(&old_data.ranges);
            self.buffer.add_id(old_data.id.unwrap());
        }
    }
    pub fn record_stale_data(&mut self, real_entities: &Vec<Entity>){
        self.data.iter_mut()
            .filter(|(k, _)| !real_entities.contains(*k))
            .for_each(|(_, v)| 
        {
            v.state = CornFieldDataState::Stale;
        });
    }
    pub fn remove_stale_data_and_update_state(&mut self){
        self.data.retain(|_, data| {
            if data.state == CornFieldDataState::Stale{
                if let Some(id) = data.id{
                    self.buffer.add_id(id);
                }
                self.buffer.add_ranges(&data.ranges);
                return false;
            }else if data.state == CornFieldDataState::Loading{
                data.state = CornFieldDataState::Loaded;
            }
            return true;
        });
    }
    pub fn init_new_data(&mut self) {
        self.data.iter_mut().filter(|(_, v)| v.state == CornFieldDataState::Unloaded)
            .for_each(|(_, data)| 
        {
            data.id = self.buffer.get_id();
            if data.id.is_none() {return;}
            data.ranges = self.buffer.get_ranges(data.get_instance_count());
            data.state = CornFieldDataState::Loading;
            self.settings.run_init_pass = true;
        });
    }
    pub fn create_settings(&mut self) -> (ComputeSettingsVector, Vec<ComputeRange>, usize){
        let mut settings_vec = ComputeSettingsVector { 
            array: [ComputeSettings::default(); 32] 
        };
        let mut ranges: Vec<ComputeRange> = vec![];
        let mut total_corn: usize = 0;
        self.data.iter().filter(|(_, v)| v.state == CornFieldDataState::Loading)
            .for_each(|(_, v)| 
        {
            settings_vec.array[v.id.unwrap() as usize] = ComputeSettings::from(v);
            total_corn += v.ranges.count();
            ranges.extend(v.ranges.convert_to_compute_vec(v.id.unwrap()));
        });
        return (settings_vec, ranges, total_corn);
    }
    pub fn create_bind_group(&mut self, render_device: &RenderDevice, layout: &BindGroupLayout){
        let mut bind_group_entries = [
            BindGroupEntry{
                binding: 0,
                resource: self.buffer.buffer.as_ref().unwrap().as_entire_binding()
            },
            BindGroupEntry{
                binding: 1,
                resource: self.settings.range_buffer.binding().unwrap()
            },
            BindGroupEntry{
                binding: 2,
                resource: self.settings.settings_buffer.binding().unwrap()
            }
        ];
        if self.buffer.needs_to_expand {
            bind_group_entries[0] = BindGroupEntry{binding: 0, resource: self.buffer.temp_buffer.as_ref().unwrap().as_entire_binding()}
        }
        self.settings.bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Init Buffer Bind Group"), 
            layout, 
            entries: &bind_group_entries
        }));
    }
    pub fn run_init(&self, render_context: &mut RenderContext, world: &World){
        if !self.settings.run_init_pass {return;}
        //get pipelines
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<InitCornBuffersPipeline>();
        //find our compute pipeline
        let compute_pipeline = pipeline_cache.get_compute_pipeline(pipeline.id);
        if compute_pipeline.is_none() {return;}
        let compute_pipeline = compute_pipeline.unwrap();
        //create compute pass
        let mut compute_pass = render_context.command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {label: Some("Initialize Corn Data Pass") });
        compute_pass.set_pipeline(&compute_pipeline);
        //Set shader inputs
        compute_pass.set_bind_group(0, self.settings.bind_group.as_ref().unwrap(), &[]);
        compute_pass.set_push_constants(0, bytemuck::cast_slice(&[self.settings.range_count as u32, 0, 0, 0]));
        compute_pass.dispatch_workgroups((self.settings.corn_count as f32 / 256.0).ceil() as u32, 1, 1);
    }
}

pub struct DynamicInstanceBuffer{
    buffer: Option<Buffer>,
    ranges: Vec<Range<u32>>,
    ids: Vec<u32>,
    temp_buffer: Option<Buffer>,
    size: usize,
    old_size: Option<usize>,
    needs_to_expand: bool
}
impl Default for DynamicInstanceBuffer{
    fn default() -> Self {
        Self{buffer: None, ranges: vec![], ids: vec![], temp_buffer: None, size: 0, old_size: None, needs_to_expand: false}
    }
}
impl DynamicInstanceBuffer{
    pub fn init(&mut self, render_device: &RenderDevice, name: &str, count: u64, stride_length: usize, usages: BufferUsages){
        self.buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some(name), 
            size: count * stride_length as u64, 
            usage: usages | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }));
        self.ids = (0..32).collect();
        self.ranges = vec![(0..count as u32)];
        self.size = count as usize;
    }
    pub fn add_ranges(&mut self, ranges: &Vec<Range<u32>>){
        self.ranges.combine(ranges);
    }
    pub fn add_id(&mut self, id: u32){
        self.ids.push(id);
    }
    pub fn finish_previous_frame_expansion(&mut self) {
        if self.needs_to_expand{
            self.buffer.as_mut().unwrap().destroy();
            self.buffer = self.temp_buffer.take();
            self.needs_to_expand = false;
            self.old_size = None;
        }
    }
    pub fn is_initialized(&self) -> bool {return self.buffer.is_some();}
    pub fn get_id(&mut self) -> Option<u32>{return self.ids.pop();}
    pub fn get_ranges(&mut self, count: u32) -> Vec<Range<u32>>{
        if self.ranges.count() < count as usize{
            self.needs_to_expand = true;
            self.old_size.get_or_insert(self.size);
            let start = self.size;
            self.size += count as usize - self.ranges.count();
            self.size += self.size/2;
            self.ranges.insert_or_extend(start as u32..self.size as u32);
        }
        self.ranges.get_ranges(count)
    }
    pub fn init_expansion(&mut self, render_device: &RenderDevice){
        self.temp_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Master Corn Buffer"), 
            size: self.size as u64*size_of::<PerCornData>() as u64, 
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST, 
            mapped_at_creation: false
        }));
    }
    pub fn run_expansion(&self, render_context: &mut RenderContext){
        if !self.needs_to_expand || self.temp_buffer.is_none(){return;}
        render_context.command_encoder().copy_buffer_to_buffer(
            self.buffer.as_ref().unwrap(), 
            0, 
            self.temp_buffer.as_ref().unwrap(), 
            0, 
            self.old_size.unwrap() as u64 * size_of::<PerCornData>() as u64
        );
    }
}

pub struct CornFieldRenderData{
    state: CornFieldDataState,
    center: Vec3,
    half_extents: Vec2,
    resolution: (usize, usize),
    height_range: Vec2,
    id: Option<u32>,
    ranges: Vec<Range<u32>>
}
impl CornFieldRenderData{
    pub fn new(center: Vec3, half_extents: Vec2, resolution: (usize, usize), height_range: Vec2) -> Self{
        Self { 
            state: CornFieldDataState::Unloaded, 
            center, 
            half_extents, 
            resolution, 
            height_range, 
            id: None, 
            ranges: vec![] 
        }
    }
    pub fn get_byte_count(&self) -> u32{
        return (self.resolution.0*self.resolution.1*size_of::<PerCornData>()) as u32;
    }
    pub fn get_instance_count(&self) -> u32{
        return (self.resolution.0*self.resolution.1) as u32;
    }
}

/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(CornFieldInitPlugin)
            .init_resource::<RenderedCornFields>();
    }
}
