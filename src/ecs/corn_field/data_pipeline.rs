use std::{ops::Range, collections::VecDeque, mem::size_of};
use bevy::{prelude::*, render::{RenderApp, Extract, Render, RenderSet, renderer::{RenderDevice, RenderContext}, render_resource::{Buffer, BufferDescriptor, BufferUsages, MapMode, BufferInitDescriptor, CachedComputePipelineId, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindingType, BufferBindingType, PipelineCache, ComputePipelineDescriptor, BindGroupEntry, BindingResource, BindGroup, BindGroupDescriptor, ComputePassDescriptor}, render_graph::{RenderGraphContext, Node, RenderGraph}}, utils::hashbrown::{HashMap, HashSet}};
use bytemuck::{Zeroable, Pod};
use super::{CornField, CornInstanceBuffer, PerCornData};

pub trait Ranges<T>{
    fn combine(&mut self, other: &Self);
    fn total(&self) -> T;
    fn take(&mut self, count: T) -> (T, Self);
    fn into_compute_ranges(&self, id: u32) -> Vec<ComputeRange>;
}
impl Ranges<u32> for Vec<Range<u32>>{
    fn combine(&mut self, other: &Self) {
        self.extend(other.to_owned().into_iter());
        self.sort_by(|a, b| {a.start.cmp(&b.start)});
        *self = self.iter().fold(vec![], |mut acc: Vec<Range<u32>>, val|{
            if acc.is_empty() {return vec![val.to_owned()];}
            if val.start <= acc.last().unwrap().end {
                let new_end = acc.last().unwrap().end.max(val.end);
                acc.last_mut().as_mut().unwrap().end = new_end;
            }else{
                acc.push(val.to_owned());
            }
            return acc;
        });
    }
    fn total(&self) -> u32 {
        self.iter().map(|range| range.end - range.start).sum()
    }
    fn take(&mut self, mut count: u32) -> (u32, Self) {
        let mut ranges: Vec<Range<u32>> = vec![];
        self.retain_mut(|range| {
            if count == 0 {return true;}
            if range.end - range.start <= count{
                ranges.push(range.to_owned());
                count -= range.end-range.start;
                return false;
            }else{
                ranges.push(range.start..(range.start+count));
                range.start += count;
                count = 0;
            }
            return true;
        });
        return (count, ranges);
    }
    fn into_compute_ranges(&self, id: u32) -> Vec<ComputeRange> {
        self.iter().scan(0, |acc, val| {
            let range = ComputeRange{
                start: val.start,
                length: val.end - val.start,
                id,
                offset: *acc
            };
            *acc += range.length;
            Some(range)
        }).collect()
    }
}

/// Represents the state of the corn instance buffer resource
#[derive(Default, Debug, PartialEq, Eq, Hash, Clone, States)]
pub enum InstanceBufferState{
    #[default]
    Uninitialized,
    Initialized,
    Destroy
}

/// Represents the state of the corn field in the render app
#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub enum CornFieldDataState{
    #[default]
    Uninitialized,
    Loading,
    Loaded,
    Stale
}

/// Represents the state of the readback buffer in the render app
#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub enum ReadbackBufferState{
    #[default]
    Disabled,
    Copying,
    Mapping
}

/// Respresents a Range for use in a compute shader
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct ComputeRange {
    start: u32,
    length: u32,
    /// 0-31, corresponds to corn field id
    id: u32,
    /// instance offset for corresponding corn field
    offset: u32
}

/// Respresents corn field settings for use in a compute shader
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct ComputeSettings {
    origin: Vec3,
    res_width: u32,
    height_width_min: Vec2,
    step: Vec2    
}
impl From::<CornFieldSettings> for ComputeSettings{
    fn from(value: CornFieldSettings) -> Self {
        let mut output = Self { 
            origin: value.center - value.half_extents.extend(0.0),
            height_width_min: Vec2::new(value.height_range.y-value.height_range.x, value.height_range.x),
            step: Vec2::new(
                value.half_extents.x*2.0/(value.resolution.0 as f32 - 1.0), 
                value.half_extents.y*2.0/(value.resolution.1 as f32 - 1.0)
            ),
            res_width: value.resolution.0 as u32
         };
         if !output.step.is_finite() || output.step.is_nan() {
            output.step = Vec2::ZERO;
            output.origin = value.center;
         }
         return output;
    }
}

// keeps track of corn fields in the render app
#[derive(Resource)]
pub struct RenderAppCornFields{
    /// hashmap of entity to corresponding render app cornfield
    corn_fields: HashMap<Entity, CornFieldData>,
    /// corn fields that were push out of the hashmap but havent been deleted yet
    displaced_stale_data: VecDeque<CornFieldData>,
    /// a struct responsible for managing the instance buffer
    corn_buffer_manager: DynamicBufferManager,
    /// wether or not to read back the instance buffer data after each change
    readback_enabled: bool
}
impl Default for RenderAppCornFields{
    fn default() -> Self {
        Self{
            corn_fields: HashMap::new(),
            displaced_stale_data: VecDeque::new(),
            corn_buffer_manager: DynamicBufferManager::new(),
            readback_enabled: false
        }
    }
}
/// Per corn field render app data
#[derive(Debug)]
pub struct CornFieldData{
    /// settings related to the structure of the corn field
    settings: CornFieldSettings,
    /// state of the data
    state: CornFieldDataState,
    /// (id, range list)
    buffer_settings: Option<(u32, Vec<Range<u32>>)>,
    defragmented_range: Option<Range<u32>>
}
/// per corn field configuration settings
#[derive(Clone, Copy, Debug)]
pub struct CornFieldSettings{
    center: Vec3,
    half_extents: Vec2,
    resolution: (u32, u32),
    height_range: Vec2,
}
impl CornFieldData{
    pub fn new(center: Vec3, half_extents: Vec2, resolution: (u32, u32), height_range: Vec2) -> Self{
        Self { 
            settings: CornFieldSettings { center, half_extents, resolution, height_range }, 
            state: CornFieldDataState::Uninitialized, 
            buffer_settings: None ,
            defragmented_range: None
        }
    }
    pub fn get_instance_count(&self) -> u64{
        (self.settings.resolution.0*self.settings.resolution.1) as u64
    }
}
impl RenderAppCornFields{
    /// adds a corn field from the main world to the struct
    pub fn add_data(&mut self, entity: &Entity, field: &CornField){
        let new_data = CornFieldData::new(
            field.center, 
            field.half_extents, 
            field.resolution, 
            field.height_range
        );
        if let Some(old_data) = self.corn_fields.insert(entity.clone(), new_data){
            self.displaced_stale_data.push_back(old_data);
        }
    }
    /// given a list of main world entities, find corn fields that no longer exist, and mark them as stale
    pub fn mark_stale_data(&mut self, real_entities: &Vec<Entity>){
        let entities: HashSet<Entity> = HashSet::from_iter(real_entities.to_owned().into_iter());
        self.corn_fields.iter_mut().for_each(|(key, val)| {
            if !entities.contains(key){val.state = CornFieldDataState::Stale;}
        });
    }
    /// returns an initialization size for the instance buffer based on the total amount of corn currently in the renderapp
    pub fn get_buffer_init_size(&self) -> u64{
        self.corn_fields.values().map(|data| data.get_instance_count()).sum()
    }
    /// initializes the instance buffer manager
    pub fn init_buffer_manager(&mut self, instance_count: u32, instance_buffer: &Buffer) {
        self.corn_buffer_manager.init(instance_count, instance_buffer);
    }
    /// deletes stale data and adds is id and ranges back into the system
    /// Also updates loading corn fields to loaded ones
    /// returns whether or not there still exists some corn
    pub fn retire_stale_data(&mut self) -> bool{
        self.corn_fields.retain(|_, val| {
            if val.state == CornFieldDataState::Loading{
                val.state = CornFieldDataState::Loaded;
                return true;
            }
            if val.state!=CornFieldDataState::Stale{return true;}
            if val.buffer_settings.is_none(){return false;}
            self.corn_buffer_manager.add_id(val.buffer_settings.as_ref().unwrap().0);
            self.corn_buffer_manager.add_stale_range(&val.buffer_settings.as_ref().unwrap().1);
            return false;
        });
        return !self.corn_fields.is_empty();
    }
    /// returns the total amount of space left in the instance buffer
    pub fn get_available_space(&self) -> u32 {
        return self.corn_buffer_manager.get_available_space();
    }
    /// gets the total number of new pieces of corn
    pub fn get_new_instances(&self) -> u32{
        self.corn_fields.iter().filter_map(|data| {
            if data.1.state != CornFieldDataState::Loading {return None;}
            return Some(data.1.get_instance_count() as u32);
        }).sum()
    }
    /// returns new instances - available space, if it is positive, tells us if we need to expand the buffer and by how much
    pub fn get_buffer_deficit(&self) -> Option<u32>{
        let open = self.get_available_space();
        let necessary = self.get_new_instances();
        if open >= necessary {return None;}
        return Some(necessary - open);
    }
    /// queues up a buffer expansion in the buffer manager
    pub fn queue_buffer_expansion(&mut self, deficit: u32, render_device: &RenderDevice){
        self.corn_buffer_manager.expand_by_at_least(deficit, render_device);
    }
    /// finishes the expansion logic, swapping the active buffer with the expanded one
    pub fn finish_expansion(&mut self, instance_buffer: &mut CornInstanceBuffer){
        self.corn_buffer_manager.finish_expansion(instance_buffer);
    }
    /// assigns new data an id from the buffer manager
    pub fn assign_new_data_ids(&mut self){
        for (_, data) in self.corn_fields.iter_mut(){
            if data.state != CornFieldDataState::Uninitialized {continue;}
            if let Some(id) = self.corn_buffer_manager.get_id(){
                data.buffer_settings = Some((id, vec![]));
                data.state = CornFieldDataState::Loading;
                self.corn_buffer_manager.needs_to_initialize = true;
            }else{
                break;
            }
        }
    }
    /// assigns new data a list of buffer ranges from the buffer manager
    pub fn assign_new_data_ranges(&mut self){
        self.corn_fields.iter_mut().filter(|(_, v)| v.state==CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            data.buffer_settings.as_mut().unwrap().1 = self.corn_buffer_manager.get_ranges(data.get_instance_count() as u32);
        });
    }
    /// returns whether or not the buffer is mostly empty: 66%
    pub fn buffer_is_sparse(&self) -> bool{
        self.corn_buffer_manager.is_sparse()
    }
    /// returns whether or not the buffer is fragmented too much
    /// Todo: get a metric for fragmentation and a limit here
    pub fn buffer_is_fragmented(&self) -> bool{return false;}
    /// queues up a buffer shrink in the buffer manager, also queues up defragmentation as well
    pub fn queue_buffer_shrink(&mut self, render_device: &RenderDevice){
        self.corn_buffer_manager.queue_defragmentation(render_device);
        self.corn_buffer_manager.queue_buffer_shrink(render_device);
        self.assign_defragmented_ranges();
    }
    /// queues up a buffer defragmentation in the buffer manager
    pub fn queue_buffer_defragmentation(&mut self, render_device: &RenderDevice){
        self.corn_buffer_manager.queue_defragmentation(render_device);
        self.assign_defragmented_ranges();
    }
    /// assigns each loaded piece of corn a new range in the defragmented buffer
    pub fn assign_defragmented_ranges(&mut self){
        self.corn_fields.iter_mut()
            .filter(|(_, v)| v.state == CornFieldDataState::Loaded || v.state == CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            data.defragmented_range = Some(self.corn_buffer_manager.get_defragmented_range(data.get_instance_count() as u32));
        });
    }
    /// finishes up defrag and shrink commands, swapping their buffers with the active one
    /// Swaps corn data ranges from olds ones to defrag'd ones
    pub fn finish_defrag_and_shrink(&mut self, instance_buffer: &mut CornInstanceBuffer){
        self.corn_fields.iter_mut()
            .filter(|(_, v)| v.state == CornFieldDataState::Loaded || v.state==CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            if let Some(ranges) = data.defragmented_range.as_ref(){
                data.buffer_settings.as_mut().unwrap().1 = vec![ranges.to_owned()];
                data.defragmented_range = None;
            }
        });
        self.corn_buffer_manager.finish_defrag_and_shrink(instance_buffer);
    }
    /// returns whether or not there is any changes to be made
    pub fn update_pending(&self) -> bool{
        if self.corn_buffer_manager.update_pending() {return true;}
        return false;
    }
    /// queues up a cpu readback operation on the buffer manager
    pub fn queue_cpu_readback(&mut self, render_device: &RenderDevice){
        if !self.readback_enabled {return;}
        if !self.update_pending() && self.corn_buffer_manager.cpu_readback_state == ReadbackBufferState::Disabled {return;}
        self.corn_buffer_manager.queue_cpu_readback(render_device);
    }
    /// finishes cpu readback, printing results to console
    pub fn finish_cpu_readback(&mut self){self.corn_buffer_manager.finish_cpu_readback();}
    /// creates compute structures that are needed
    pub fn create_structures(&mut self, render_device: &RenderDevice){
        let new_data: Vec<(u32, Vec<Range<u32>>, CornFieldSettings)> = self.corn_fields
            .iter()
            .filter_map(|(_, v)| {
                if v.state!=CornFieldDataState::Loading {return None;}
                if let Some((id, ranges)) = v.buffer_settings.as_ref(){
                    return Some((id.to_owned(), ranges.to_owned(), v.settings));
                }
                return None;
            }).collect();
        self.corn_buffer_manager.create_structures(new_data, render_device);
        let old_ranges: Vec<(u32, Vec<Range<u32>>)> = self.corn_fields
            .iter().filter_map(|(_, v)| {
                if v.state!=CornFieldDataState::Loading && v.state != CornFieldDataState::Loaded {return None;}
                if let Some((id, ranges)) = v.buffer_settings.as_ref(){
                    return Some((id.to_owned(), ranges.to_owned()));
                }
                return None;
            }).collect();
        let offsets: Vec<(u32, u32)> = self.corn_fields
        .iter().filter_map(|(_, v)| {
            if v.state!=CornFieldDataState::Loading && v.state != CornFieldDataState::Loaded {return None;}
            if let Some((id, _)) = v.buffer_settings.as_ref(){
                if let Some(ranges2) = v.defragmented_range.as_ref(){
                    return Some((id.to_owned(), ranges2.start));
                }
            }
            return None;
        }).collect();
        self.corn_buffer_manager.create_defrag_structures(old_ranges, offsets, render_device);
    }
    /// Runs the compute pass
    pub fn run_compute_pass(&self, render_context: &mut RenderContext, world: &World){
        self.corn_buffer_manager.run_compute_pass(render_context, world);
    }
    /// completely resets the struct
    pub fn destroy(&mut self){
        self.corn_fields = HashMap::new();
        self.displaced_stale_data = VecDeque::new();
        self.corn_buffer_manager.destroy();
        self.corn_buffer_manager = DynamicBufferManager::new();
    }
}
/// Buffer Management Struct
#[derive(Default)]
pub struct DynamicBufferManager{
    /// Usually points to the same buffer as corn instance buffer, unless it needs to change size
    initialization_buffer: Option<Buffer>,
    /// Contains all available, unused id's, 0-31 initially
    ids: Vec<u32>,
    /// list of all unused ranges in the instance buffer
    ranges: Vec<Range<u32>>,
    /// list of ranges that contain stale data
    stale_ranges: Vec<Range<u32>>,
    /// whether or not there is new data to initialize
    needs_to_initialize: bool,
    /// expanded index buffer, used when we expand the instance buffer
    expanded_index_buffer: Option<Buffer>,
    /// when we expand the instance buffer we keep the original here for when we need to copy its data to the larger buffer
    original_buffer: Option<Buffer>,
    /// whether or not we need to expand the buffer
    needs_to_expand: bool,
    /// current working size of the buffer: expanding: size of expanded buffer, shrinking: size of shrunken buffer
    active_size: u32,
    /// whether or not we need to shrink the buffer 
    /// If we shrink we also need to defragment
    needs_to_shrink: bool,
    /// whether or not we need to defragment the buffer
    needs_to_defragment: bool,
    /// buffer where we place defragmented data
    defragmented_buffer: Option<Buffer>,
    /// shrunken buffer where we place data
    shrunken_buffer: Option<Buffer>,
    /// shrunken index buffer for use when we finally swap our shrunken buffers and active ones
    shrunken_index_buffer: Option<Buffer>,
    /// list of ranges of the defragmented buffer.
    /// ranges are assigned to all data when we queue defragmentation
    /// realistically, this will only ever have 1 item since portions are only ever taken out, not readded in
    defragmented_ranges: Option<Vec<Range<u32>>>,
    /// state of the readback buffer. readback takes 3 frames (copy, map, read) so we need this to track that
    cpu_readback_state: ReadbackBufferState,
    /// the buffer we copy instance data to when we read back to the cpu
    readback_buffer: Option<Buffer>,
    /// holds compute shader structures necessary for dispatch such as bind groups and constant buffers
    compute_structures: ComputeStructures
}
impl DynamicBufferManager{
    ///returns empty new struct
    pub fn new() -> Self{
        Self { 
            initialization_buffer: None, 
            ids: vec![], 
            ranges: vec![], 
            stale_ranges: vec![], 
            needs_to_initialize: false, 
            expanded_index_buffer: None, 
            original_buffer: None, 
            needs_to_expand: false, 
            active_size: 0, 
            needs_to_shrink: false, 
            needs_to_defragment: false, 
            defragmented_buffer: None, 
            shrunken_buffer: None, 
            shrunken_index_buffer: None, 
            defragmented_ranges: None, 
            cpu_readback_state: ReadbackBufferState::Disabled, 
            readback_buffer: None, 
            compute_structures: ComputeStructures { 
                ranges_buffer: None, 
                settings_buffer: None, 
                defrag_ranges_buffer: None, 
                defrag_offset_buffer: None, 
                init_bind_group: None, 
                defrag_bind_group: None,
                total_init_corn: 0, 
                total_defrag_corn: 0 
            } 
        }
    }
    /// initializes the buffer manager
    pub fn init(&mut self, instance_count: u32, instance_buffer: &Buffer){
        self.ids = (0..32).collect();
        self.ranges = vec![(0..instance_count)];
        self.active_size = instance_count;
        self.needs_to_expand = false;
        self.initialization_buffer = Some(instance_buffer.to_owned());
        self.expanded_index_buffer = None;
    }
    /// adds an id to the available ids
    pub fn add_id(&mut self, id: u32){
        self.ids.push(id);
    }
    /// adds a list of ranges to the list of stale ranges
    pub fn add_stale_range(&mut self, stale_ranges: &Vec<Range<u32>>){
        self.stale_ranges.combine(stale_ranges);
    }
    /// converts stale ranges to regular ones
    pub fn convert_stale_ranges(&mut self){
        self.ranges.combine(&self.stale_ranges);
        self.stale_ranges = vec![];
    }
    /// returns the total available space one the instace buffer
    pub fn get_available_space(&self) -> u32{
        return self.ranges.total() + self.stale_ranges.total();
    }
    /// queues up buffer expansion by count size
    /// expansions expand the buffer by 1.5 times the necessary size
    pub fn expand_by_at_least(&mut self, count: u32, render_device: &RenderDevice){
        self.needs_to_expand = true;
        let original_size: u32 = self.active_size;
        self.active_size += count;
        self.active_size += self.active_size/2;
        self.original_buffer = self.initialization_buffer.clone();
        self.initialization_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }));
        self.expanded_index_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Index Buffer"), 
            size: self.active_size as u64 * 4, 
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX, 
            mapped_at_creation: false
        }));
        self.ranges.combine(&vec![original_size..self.active_size]);
    }
    /// finishes expansions commands by swapping expanded buffer with active one
    pub fn finish_expansion(&mut self, instance_buffer: &mut CornInstanceBuffer){
        if !self.needs_to_expand {return;}
        instance_buffer.swap_data_buffers(
            self.initialization_buffer.as_ref().unwrap(), 
            self.expanded_index_buffer.as_ref().unwrap(), 
            self.active_size);
        self.needs_to_expand = false;
        self.expanded_index_buffer = None;
        self.original_buffer = None;
    }
    /// gets an id from the available id's if there is one
    pub fn get_id(&mut self) -> Option<u32>{
        self.ids.pop()
    }
    /// gets a list of ranges totaling count from the available ranges
    pub fn get_ranges(&mut self, count: u32) -> Vec<Range<u32>>{
        let (remaining, mut ranges) = self.stale_ranges.take(count);
        ranges.combine(&self.ranges.take(remaining).1);
        return ranges;
    }
    /// returns whether or not the buffer is sparse
    pub fn is_sparse(&self) -> bool{
        if self.needs_to_expand {return false;}
        //buffer is sparse if less than 1/3 of the buffer stores values
        self.ranges.total() + self.stale_ranges.total() > 2*self.active_size/3
    }
    /// queues up a defragmentation operation
    pub fn queue_defragmentation(&mut self, render_device: &RenderDevice){
        self.needs_to_defragment = true;
        self.defragmented_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }));
        self.defragmented_ranges = Some(vec![0..self.active_size]);
    }
    /// queues up a buffer shrink operation
    pub fn queue_buffer_shrink(&mut self, render_device: &RenderDevice){
        self.needs_to_shrink = true;
        self.active_size = self.active_size - self.stale_ranges.total() - self.ranges.total();
        self.active_size += self.active_size / 2;
        self.shrunken_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }));
        self.shrunken_index_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Index Buffer"), 
            size: self.active_size as u64 * 4, 
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX, 
            mapped_at_creation: false
        }));
        self.defragmented_ranges = Some(vec![0..self.active_size]);
    }
    /// returns a range of values totaling count from the new defragmented buffer
    pub fn get_defragmented_range(&mut self, count: u32) -> Range<u32>{
        // Assumes that we will only get one
        self.defragmented_ranges.as_mut().unwrap().take(count).1[0].to_owned()
    }
    /// finishes defrag and shrink operations
    /// if we shrunk, shrunken buffer gets swapped with the active one
    /// other wise if we defraged anyway we swap defrag buffer
    pub fn finish_defrag_and_shrink(&mut self, instance_buffer: &mut CornInstanceBuffer){
        if self.needs_to_shrink{
            instance_buffer.swap_data_buffers(
                self.shrunken_buffer.as_ref().unwrap(), 
                self.shrunken_index_buffer.as_ref().unwrap(), 
                self.active_size
            );
            self.needs_to_shrink = false;
            self.needs_to_defragment = false;
            self.initialization_buffer = self.shrunken_buffer.clone();
            self.shrunken_buffer = None;
            self.defragmented_buffer.as_mut().unwrap().destroy();
            self.defragmented_buffer = None;
            self.shrunken_index_buffer = None;
            self.ranges = self.defragmented_ranges.as_ref().unwrap().to_owned();
            self.defragmented_ranges = None;
        }else if self.needs_to_defragment{
            instance_buffer.swap_only_data_buffer(
                self.defragmented_buffer.as_ref().unwrap()
            );
            self.initialization_buffer = self.defragmented_buffer.clone();
            self.needs_to_shrink = false;
            self.needs_to_defragment = false;
            self.shrunken_buffer = None;
            self.defragmented_buffer = None;
            self.shrunken_index_buffer = None;
            self.ranges = self.defragmented_ranges.as_ref().unwrap().to_owned();
            self.defragmented_ranges = None;
        }
    }
    /// queues up a cpu readback operation
    pub fn queue_cpu_readback(&mut self, render_device: &RenderDevice){
        if self.cpu_readback_state != ReadbackBufferState::Disabled{return;}
        self.cpu_readback_state = ReadbackBufferState::Copying;
        self.readback_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Readback Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
            mapped_at_creation: false
        }));
    }
    /// returns whether or not there are any updates to the buffer pending
    pub fn update_pending(&self) -> bool{
        self.needs_to_defragment || self.needs_to_expand || self.needs_to_initialize
    }
    /// finishes cpu readback
    pub fn finish_cpu_readback(&mut self){
        if self.cpu_readback_state == ReadbackBufferState::Disabled{return;}
        if self.cpu_readback_state == ReadbackBufferState::Copying{
            self.cpu_readback_state = ReadbackBufferState::Mapping;
            self.readback_buffer.as_ref().unwrap().slice(..).map_async(MapMode::Read, |_|{});
            return;
        }
        let raw = self.readback_buffer.as_ref().unwrap()
            .slice(..).get_mapped_range()
            .iter().map(|v| *v).collect::<Vec<u8>>();
        let data = bytemuck::cast_slice::<u8, PerCornData>(raw.as_slice()).to_vec();
        for corn in data{
            println!("{:?}", corn);
        }
        println!("");
        self.readback_buffer.as_mut().unwrap().destroy();
        self.readback_buffer = None;
        self.cpu_readback_state = ReadbackBufferState::Disabled;
    }
    /// creates the constant buffers needed by the compute passes
    pub fn create_structures(&mut self, new_data: Vec<(u32, Vec<Range<u32>>, CornFieldSettings)>, render_device: &RenderDevice){
        self.compute_structures.create_structures(new_data, self.stale_ranges.clone(), render_device);
    }
    /// creates the buffers needed by the defrag pass
    pub fn create_defrag_structures(&mut self, old_ranges: Vec<(u32, Vec<Range<u32>>)>, offsets: Vec<(u32, u32)>, render_device: &RenderDevice){
        if !self.needs_to_defragment {return;}
        self.compute_structures.create_defrag_structures(old_ranges, offsets, render_device);
    }
    /// creates the bind groups for both compute passes, init and defrag
    pub fn create_bind_group(&mut self, render_device: &RenderDevice, data_pipeline: &CornDataPipeline){
        if self.initialization_buffer.is_none() {return;}
        if self.stale_ranges.len() > 0 || self.needs_to_initialize{
            self.compute_structures.create_init_bind_group(
                self.initialization_buffer.as_ref().unwrap(), 
                render_device, data_pipeline);
        }
        if self.needs_to_defragment {
            self.compute_structures.create_defrag_bind_group(
                self.initialization_buffer.as_ref().unwrap(), 
                self.defragmented_buffer.as_ref().unwrap(),
                render_device, 
                data_pipeline
            );
        }
    }
    /// runs the compute pass
    pub fn run_compute_pass(&self, render_context: &mut RenderContext, world: &World){
        if self.needs_to_expand{
            render_context.command_encoder().copy_buffer_to_buffer(
                self.original_buffer.as_ref().unwrap(), 
                0, 
                self.initialization_buffer.as_ref().unwrap(), 
                0, 
                self.original_buffer.as_ref().unwrap().size() as u64
            );
        }
        //get pipelines
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<CornDataPipeline>();
        if self.needs_to_initialize{
            //find our compute pipeline
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.init_id){
                //create compute pass
                let mut compute_pass = render_context.command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {label: Some("Initialize Corn Data Pass") });
                compute_pass.set_pipeline(&compute_pipeline);
                //Set shader inputs
                compute_pass.set_bind_group(0, self.compute_structures.init_bind_group.as_ref().unwrap(), &[]);
                compute_pass.dispatch_workgroups((self.compute_structures.total_init_corn as f32 / 256.0).ceil() as u32, 1, 1);
            }
        }
        if self.needs_to_defragment{
            //find our compute pipeline
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.defrag_id){
                //create compute pass
                let mut compute_pass = render_context.command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {label: Some("Defrag Corn Data Pass") });
                compute_pass.set_pipeline(&compute_pipeline);
                //Set shader inputs
                compute_pass.set_bind_group(0, self.compute_structures.defrag_bind_group.as_ref().unwrap(), &[]);
                compute_pass.dispatch_workgroups((self.compute_structures.total_defrag_corn as f32 / 256.0).ceil() as u32, 1, 1);
            }
        }
        if self.needs_to_shrink{
            render_context.command_encoder().copy_buffer_to_buffer(
                self.defragmented_buffer.as_ref().unwrap(), 
                0, 
                self.shrunken_buffer.as_ref().unwrap(), 
                0, 
                self.shrunken_buffer.as_ref().unwrap().size() as u64
            );
        }
        if self.cpu_readback_state == ReadbackBufferState::Copying{
            if self.needs_to_shrink{
                render_context.command_encoder().copy_buffer_to_buffer(
                    self.shrunken_buffer.as_ref().unwrap(), 
                    0, 
                    self.readback_buffer.as_ref().unwrap(), 
                    0, 
                    self.readback_buffer.as_ref().unwrap().size() as u64
                );
            }else{
                render_context.command_encoder().copy_buffer_to_buffer(
                    self.initialization_buffer.as_ref().unwrap(), 
                    0, 
                    self.readback_buffer.as_ref().unwrap(), 
                    0, 
                    self.readback_buffer.as_ref().unwrap().size() as u64
                );
            }
        }
    }
    /// destroys the buffers
    pub fn destroy(&mut self){
    if let Some(buffer) = self.expanded_index_buffer.as_ref(){buffer.destroy(); self.expanded_index_buffer = None;}
    if let Some(buffer) = self.original_buffer.as_ref(){buffer.destroy(); self.original_buffer = None;}
    if let Some(buffer) = self.defragmented_buffer.as_ref(){buffer.destroy(); self.defragmented_buffer = None;}
    if let Some(buffer) = self.shrunken_buffer.as_ref(){buffer.destroy(); self.shrunken_buffer = None;}
    if let Some(buffer) = self.shrunken_index_buffer.as_ref(){buffer.destroy(); self.shrunken_index_buffer = None;}
    if let Some(buffer) = self.readback_buffer.as_ref(){buffer.destroy(); self.readback_buffer = None}
    self.compute_structures.destroy();
    }
}
#[derive(Default)]
/// Stores per frame compute shader values
pub struct ComputeStructures{
    /// used in init, list of new and stale data ranges
    ranges_buffer: Option<Buffer>,
    /// used in init, list of per corn field settings
    settings_buffer: Option<Buffer>,
    /// used in defrag, list of all data ranges
    defrag_ranges_buffer: Option<Buffer>,
    /// used in defrag, list of per corn field offsets in defragmented buffer
    defrag_offset_buffer: Option<Buffer>,
    /// bind group for the init pass
    init_bind_group: Option<BindGroup>,
    /// bind group for the defrag pass
    defrag_bind_group: Option<BindGroup>,
    /// total amount of new and stale data
    total_init_corn: u32,
    /// total amount of data to copy in defrag stage
    total_defrag_corn: u32
}
impl ComputeStructures{
    pub fn create_structures(
        &mut self, 
        new_data: Vec<(u32, Vec<Range<u32>>, CornFieldSettings)>, 
        stale_data: Vec<Range<u32>>,
        render_device: &RenderDevice
    ){
        if let Some(buffer) = self.ranges_buffer.as_mut(){buffer.destroy(); self.ranges_buffer = None;}
        if let Some(buffer) = self.settings_buffer.as_mut(){buffer.destroy(); self.settings_buffer = None;}
        let mut ranges: Vec<ComputeRange> = vec![];
        let mut settings: Vec<ComputeSettings> = vec![ComputeSettings::default(); 32];
        for (id, new_ranges, new_settings) in new_data{
            ranges.extend(new_ranges.into_compute_ranges(id));
            settings[id as usize] = new_settings.into();
        }
        ranges.extend(stale_data.into_compute_ranges(32));
        self.total_init_corn = ranges.iter().map(|r| r.length).sum();
        self.ranges_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
            label: Some("Corn Ranges Buffer"), 
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&ranges[..])
        }));
        self.settings_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
            label: Some("Corn Settings Buffer"), 
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&settings[..])
        }));
    }
    pub fn create_defrag_structures(
        &mut self, 
        old_ranges: Vec<(u32, Vec<Range<u32>>)>,
        offsets: Vec<(u32, u32)>,
        render_device: &RenderDevice
    ){
        if let Some(buffer) = self.defrag_offset_buffer.as_mut(){buffer.destroy(); self.defrag_offset_buffer = None;}
        if let Some(buffer) = self.defrag_ranges_buffer.as_mut(){buffer.destroy(); self.defrag_ranges_buffer = None;}
        let mut ranges: Vec<ComputeRange> = vec![];
        let mut sum: u32 = 0;
        for (id, old_range) in old_ranges{
            sum += old_range.total();
            ranges.extend(old_range.into_compute_ranges(id));
        }
        self.total_defrag_corn = sum;
        let mut defrag_offsets: [u32; 128] = [0; 128];
        for (id, offset) in offsets{
            defrag_offsets[id as usize*4] = offset;
        }
        self.defrag_ranges_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
            label: Some("Corn Defrag Ranges Buffer"), 
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&ranges[..])
        }));
        self.defrag_offset_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
            label: Some("Corn Defrag Offsets Buffer"), 
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&defrag_offsets)
        }));
    }
    pub fn create_init_bind_group(
        &mut self, 
        instance_buffer: &Buffer,
        render_device: &RenderDevice, 
        data_pipeline: &CornDataPipeline
    ){
        let init_bind_group = [
            BindGroupEntry{
                binding: 0,
                resource: BindingResource::Buffer(instance_buffer.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 1,
                resource: BindingResource::Buffer(self.settings_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 2,
                resource: BindingResource::Buffer(self.ranges_buffer.as_ref().unwrap().as_entire_buffer_binding())
            }            
        ];
        self.init_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Init Buffer Bind Group"), 
            layout: &data_pipeline.init_bind_group, 
            entries: &init_bind_group
        }));
    }
    pub fn create_defrag_bind_group(
        &mut self,
        instance_buffer: &Buffer,
        defrag_buffer: &Buffer,
        render_device: &RenderDevice,
        data_pipeline: &CornDataPipeline
    ){
        let defrag_bind_group = [
            BindGroupEntry{
                binding: 0,
                resource: BindingResource::Buffer(self.defrag_ranges_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 1,
                resource: BindingResource::Buffer(self.defrag_offset_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 2,
                resource: BindingResource::Buffer(defrag_buffer.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 3,
                resource: BindingResource::Buffer(instance_buffer.as_entire_buffer_binding())
            }
        ];
        self.defrag_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Defrag Bind Group"), 
            layout: &data_pipeline.defrag_bind_group, 
            entries: &defrag_bind_group
        }));
    }
    pub fn destroy(&mut self){
        if let Some(buffer) = self.ranges_buffer.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.settings_buffer.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.defrag_offset_buffer.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.defrag_ranges_buffer.as_ref() {buffer.destroy();}
    }
}

/// Pipeline struct 
#[derive(Resource)]
pub struct CornDataPipeline{
    pub init_id: CachedComputePipelineId,
    pub defrag_id: CachedComputePipelineId,
    init_bind_group: BindGroupLayout,
    defrag_bind_group: BindGroupLayout
}
impl FromWorld for CornDataPipeline {
    fn from_world(world: &mut World) -> Self {
        let init_bind_group = world.resource::<RenderDevice>().create_bind_group_layout(
            &BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Storage { read_only: false }, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Uniform, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Storage { read_only: true }, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    }
                ],
            }
        );
        let defrag_bind_group = world.resource::<RenderDevice>().create_bind_group_layout(
            &BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Storage { read_only: true }, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Uniform, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Storage { read_only: false }, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Storage { read_only: true }, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    }
                ],
            }
        );
        let init_shader = world
            .resource::<AssetServer>()
            .load("shaders/corn_data_init.wgsl");
        let defrag_shader = world
            .resource::<AssetServer>()
            .load("shaders/corn_data_defrag.wgsl");
        let pipeline_cache = world.resource::<PipelineCache>();
        let init_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Initialize Corn Pipeline".into()),
            layout: vec![init_bind_group.clone()],
            push_constant_ranges: vec![],
            shader: init_shader,
            shader_defs: vec![],
            entry_point: "init".into(),
        });
        let defrag_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Initialize Corn Pipeline".into()),
            layout: vec![defrag_bind_group.clone()],
            push_constant_ranges: vec![],
            shader: defrag_shader,
            shader_defs: vec![],
            entry_point: "defragment".into(),
        });
        Self{init_id: init_pipeline, defrag_id: defrag_pipeline, init_bind_group, defrag_bind_group}
    }
}

/// Always runs, copying corn fields from the main world to the render world
pub fn extract_corn_fields(
    corn_fields: Extract<Query<(Entity, Ref<CornField>)>>,
    mut corn_res: ResMut<RenderAppCornFields>
){
    corn_fields.iter().filter(|(_, f)| f.is_changed()).for_each(|(e, f)| {
        corn_res.add_data(&e, f.as_ref());
    });
    let entities: Vec<Entity> = corn_fields.iter().map(|(e, _)| e).collect();
    corn_res.mark_stale_data(&entities);
}
/// Runs if the instance buffer is not initialized, and creates it when there is corn field data in the game
pub fn initialize_instance_buffer(
    mut instance_buffer: ResMut<CornInstanceBuffer>,
    mut corn_fields: ResMut<RenderAppCornFields>,
    render_device: Res<RenderDevice>,
    mut next_state: ResMut<NextState<InstanceBufferState>>
){
    if corn_fields.corn_fields.is_empty(){return;}
    if instance_buffer.initialize_data(&render_device, corn_fields.get_buffer_init_size()){
        corn_fields.init_buffer_manager(instance_buffer.get_instance_count(), instance_buffer.get_instance_buffer().unwrap());
        next_state.set(InstanceBufferState::Initialized);
    }
}
/// Finish up Previous Frames work
pub fn finish_previous_frame_work(
    mut corn_fields: ResMut<RenderAppCornFields>,
    mut instance_buffer: ResMut<CornInstanceBuffer>
){
    // convert stale ranges to regular ones, since current stale ranges were fixed last frame
    // has to happen before finish shrink
    corn_fields.corn_buffer_manager.convert_stale_ranges();
    corn_fields.finish_expansion(instance_buffer.as_mut());
    corn_fields.finish_defrag_and_shrink(instance_buffer.as_mut());
    corn_fields.finish_cpu_readback();
    corn_fields.corn_buffer_manager.needs_to_initialize = false;
}
/// Setup current frame new data and stale data, as well as buffer expansion, shrinking, and defregmentation
/// Also sets up cpu readback if enabled
pub fn manage_corn_data(
    mut corn_fields: ResMut<RenderAppCornFields>,
    render_device: Res<RenderDevice>,
    mut next_state: ResMut<NextState<InstanceBufferState>>
){
    //Free up stale data ranges and ids, putting the ranges into a list for future flagging
    // if no more data exists, reset everything and set instance buffer state to destroy
     if !corn_fields.retire_stale_data() {
        corn_fields.destroy();
        next_state.set(InstanceBufferState::Destroy)
     };
    //Let new data grab an id from the buffer manager
    //updates state to loading if it got an id
    corn_fields.assign_new_data_ids();
    //If need be, queue up a buffer expansion. doesnt include data that cant get an id
    if let Some(deficit) = corn_fields.get_buffer_deficit(){
        corn_fields.queue_buffer_expansion(deficit, &render_device);
    }
    //assign ranges to new data
    corn_fields.assign_new_data_ranges();
    //Queue up defragmentation and shrinkin if the buffer is too large
    if corn_fields.buffer_is_sparse(){
        corn_fields.queue_buffer_shrink(&render_device);
    }else if corn_fields.buffer_is_fragmented(){
        corn_fields.queue_buffer_defragmentation(&render_device);
    }
    //readback data from the gpu if needed
    corn_fields.queue_cpu_readback(&render_device);
}
/// Sets up constant buffers for the initialization and defragementation shaders
pub fn prepare_compute_structures(
    mut corn_fields: ResMut<RenderAppCornFields>,
    render_device: Res<RenderDevice>,
    pipeline: Res<CornDataPipeline>,
){
    if !corn_fields.update_pending() {return;}
    corn_fields.create_structures(&render_device);
    corn_fields.corn_buffer_manager.create_bind_group(&render_device, &pipeline);
}
/// destroys the corn instance buffer and sets instance buffer state to uninitialized
pub fn destroy_buffer(
    mut instance_buffer: ResMut<CornInstanceBuffer>,
    mut next_state: ResMut<NextState<InstanceBufferState>>
){
    instance_buffer.destroy();
    next_state.set(InstanceBufferState::Uninitialized);
}
pub struct CornDataPipelineNode{}
impl Node for CornDataPipelineNode{
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        //get corn fields resource
        let corn_res = world.get_resource::<RenderAppCornFields>();
        if corn_res.is_none() {return Ok(());}
        let corn_res = corn_res.unwrap();
        //expand the buffer
        corn_res.run_compute_pass(render_context, world);
        return Ok(());
    }
}

/// Main plugin for the corn data pipeline
pub struct CornFieldDataPipelinePlugin;
impl Plugin for CornFieldDataPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.get_sub_app_mut(RenderApp).unwrap()
            .add_state::<InstanceBufferState>()
            .init_resource::<RenderAppCornFields>()
            .add_systems(ExtractSchedule, extract_corn_fields)
            .add_systems(Render, (
                initialize_instance_buffer.in_set(RenderSet::Prepare).run_if(in_state(InstanceBufferState::Uninitialized)),
                (
                    finish_previous_frame_work,
                    manage_corn_data.after(finish_previous_frame_work),
                    prepare_compute_structures.after(manage_corn_data)
                ).in_set(RenderSet::Prepare).run_if(in_state(InstanceBufferState::Initialized)),
                (
                    destroy_buffer.run_if(in_state(InstanceBufferState::Destroy)),
                    apply_state_transition::<InstanceBufferState>.after(destroy_buffer)
                ).in_set(RenderSet::Cleanup)
            ))
        .world.get_resource_mut::<RenderGraph>().unwrap()
            .add_node("Corn Buffer Data Pipeline", CornDataPipelineNode{});
    }
    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CornDataPipeline>();
    }
}