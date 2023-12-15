use std::{ops::Range, collections::VecDeque, mem::size_of, sync::{Arc, Mutex}};
use bevy::{prelude::*, render::{RenderApp, Extract, Render, RenderSet, renderer::{RenderDevice, RenderContext}, render_resource::{Buffer, BufferDescriptor, BufferUsages, MapMode, BufferInitDescriptor, CachedComputePipelineId, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindingType, BufferBindingType, PipelineCache, ComputePipelineDescriptor, BindGroupEntry, BindingResource, BindGroup, BindGroupDescriptor, ComputePassDescriptor}, render_graph::{RenderGraphContext, Node, RenderGraph}}, utils::hashbrown::{HashMap, HashSet}};
use bytemuck::{Zeroable, Pod};
use wgpu::Maintain;
use super::{CornField, CornInstanceBuffer, PerCornData};
use bitflags::*;
/*==================
    Extract Phase:
  ==================*/
/// ## Always runs during the extract schedule
/// ### 2 Jobs: 
/// - Copying new data to the RenderAppCornFields resource
/// - Flagging deleted data as stale
pub fn extract_corn_fields(
    corn_fields: Extract<Query<(Entity, Ref<CornField>)>>,
    mut corn_res: ResMut<RenderAppCornFields>
){
    corn_fields.iter().filter(|(_, f)| f.is_changed())
        .for_each(|(e, f)| 
    {
        corn_res.add_corn_field(&e, f.as_ref());
    });
    let entities: Vec<Entity> = corn_fields.iter().map(|(e, _)| e).collect();
    corn_res.flag_stale_data(&entities);
}
/*==================
    Prepare Phase:
  ==================*/
/// ## Initializes Corn Instance Buffer
/// 
/// Runs during the prepare phase is the instance buffer is not yet initialized.
/// 
/// Only initializes it if there is corn fields wanting to be rendered.
pub fn initialize_instance_buffer(
    mut instance_buffer: ResMut<CornInstanceBuffer>,
    mut corn_fields: ResMut<RenderAppCornFields>,
    render_device: Res<RenderDevice>,
    mut next_state: ResMut<NextState<InstanceBufferState>>
){
    if corn_fields.corn_fields.is_empty(){return;}
    if instance_buffer.initialize_data(&render_device, corn_fields.get_buffer_init_size()){
        corn_fields.corn_buffer_manager.init(
            instance_buffer.get_instance_count(), 
            instance_buffer.get_instance_buffer().unwrap());
        next_state.set(InstanceBufferState::Initialized);
    }
}
/// ## Prepares corn data pipeline for Render Phase
/// 
/// Runs in the prepare phase
/// 
/// ### Tasks:
/// - assign new corn fields id's if any are available
/// - Queue up buffer expansion if necessary
/// - prepare new corn data for initialization
/// - Queue up defragmentation and shrinking operations if necessary
/// - Queue up CPU readback if enabled
/// - Create compute pipeline structures such as constant buffers and bind groups
pub fn prepare_corn_data(
    mut corn_fields: ResMut<RenderAppCornFields>,
    render_device: Res<RenderDevice>,
    pipeline: Res<CornDataPipeline>
){
    // add stale data ranges and id's back into the system
    corn_fields.retire_stale_data();
    // if no corn exists, exit early as any operations would panic
    if corn_fields.corn_fields.is_empty(){return;}
    // start loading uninitialized data
    corn_fields.init_new_data_load();
    // if our loading corn fields need more space than is avaiable, queue up buffer expansion
    if let Some(overflow) = 
        corn_fields.get_loading_corn_count().checked_sub(
        corn_fields.corn_buffer_manager.get_available_space()+1
    ){
        corn_fields.queue_expansion(overflow+1, render_device.as_ref());
    }
    // assign ranges of the buffer to our loading corn fields
    corn_fields.assign_new_data_ranges();
    if corn_fields.get_loading_corn_count() > 0 || corn_fields.corn_buffer_manager.stale_ranges.total() > 0{
        corn_fields.queue_init(render_device.as_ref(), pipeline.as_ref());
    }
    //check for fragmentation or sparseness, and then queue up the respective fixes
    if corn_fields.corn_buffer_manager.is_sparse(){
        corn_fields.queue_buffer_shrink(render_device.as_ref(), pipeline.as_ref());
    }else if corn_fields.buffer_is_fragmented(){
        corn_fields.queue_buffer_defragmentation(render_device.as_ref(), pipeline.as_ref());
    }
    // queue up cpu readback if enabled
    if corn_fields.readback_enabled{
        corn_fields.queue_cpu_readback(render_device.as_ref());
    }
}
/*================
    Render Phase
  ================*/
/// ### Added to the rendergraph as an asynchronous step
/// - run function is called by the render phase at some point
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
        corn_res.corn_buffer_manager.run_compute_pass(render_context, world);
        return Ok(());
    }
}
/*=================
    Cleanup Phase
  =================*/
/// ## Cleans up operations from the frame
/// ### Tasks:
/// - If the resource is empty, reset it and set the next state to uninitialized
/// - Expand: swap new instance buffer with instance buffer resource
/// - Init: turn stale ranges into real ones
/// - Init: turn loading corn into loaded corn
/// - Defrag: turn defrag ranges into official ranges
/// - Defrag: swap defrag and instance buffer
/// - Readback: Read cpu buffer and then destroy it
/// - reset planned actions
/// - reset compute structures
pub fn cleanup_corn_data(
    corn_fields: &mut RenderAppCornFields,
    next_state: &mut NextState<InstanceBufferState>,
    instance_buffer: &mut CornInstanceBuffer,
    render_device: &RenderDevice
){
    if corn_fields.corn_fields.is_empty(){
        corn_fields.destroy();
        instance_buffer.destroy();
        next_state.set(InstanceBufferState::Uninitialized);
        return;
    }
    if corn_fields.corn_buffer_manager.planned_actions.contains(BufferActions::EXPAND){
        corn_fields.corn_buffer_manager.finish_expansion(instance_buffer, render_device);
    }
    if corn_fields.corn_buffer_manager.planned_actions.contains(BufferActions::INITIALIZE){
        corn_fields.corn_buffer_manager.convert_stale_ranges();
        corn_fields.finish_loading_corn();
    }
    if corn_fields.corn_buffer_manager.planned_actions.contains(BufferActions::DEFRAGMENT){
        corn_fields.corn_buffer_manager.finish_defragment(instance_buffer, render_device);
        corn_fields.finish_defragment();
    }
    if corn_fields.corn_buffer_manager.planned_actions.contains(BufferActions::READBACK){
        corn_fields.corn_buffer_manager.finish_cpu_readback(render_device);
    }
    corn_fields.corn_buffer_manager.per_frame_reset();
}
/*======================
    Main Functionality
  ======================*/
/// Keeps track of corn fields in the render app
#[derive(Resource)]
pub struct RenderAppCornFields{
    /// Hashmap of entity => corresponding render app cornfield
    corn_fields: HashMap<Entity, CornFieldData>,
    /// Corn fields that were pushed out of the hashmap but haven't been deleted yet
    displaced_stale_data: VecDeque<CornFieldData>,
    /// Struct responsible for managing the instance buffer
    corn_buffer_manager: DynamicBufferManager,
    /// Whether or not to read back the instance buffer data after each change
    readback_enabled: bool,
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
impl RenderAppCornFields{
//==========================================================================================
    /// ### Adds a corn field from the main world into the resource
    /// - if an corn field already exists in the resource, it is placed into a temporary storage and queued for deletion
    pub fn add_corn_field(&mut self, entity: &Entity, field: &CornField){
        let new_data = CornFieldData::new(
            field.center, 
            field.half_extents, 
            field.dist_between, 
            field.height_range,
            field.rand_offset_factor
        );
        if let Some(old_data) = self.corn_fields.insert(entity.clone(), new_data){
            self.displaced_stale_data.push_back(old_data);
        }
    }
    /// ### Flags stale data based on a set of real entities
    pub fn flag_stale_data(&mut self, real_entities: &Vec<Entity>){
        let entities: HashSet<Entity> = HashSet::from_iter(real_entities.to_owned().into_iter());
        self.corn_fields.iter_mut().for_each(|(key, val)| {
            if !entities.contains(key){val.state = CornFieldDataState::Stale;}
        });
    }
//==========================================================================================
    /// ### Returns the size to initialize the instance buffer at
    /// - uses the total amount of corn in the resource as its return value
    pub fn get_buffer_init_size(&self) -> u64{
        self.corn_fields.values().map(|data| data.get_instance_count()).sum()
    }
//==========================================================================================
    /// ### Deletes stale data and adds the id and ranges back into the system
    pub fn retire_stale_data(&mut self){
        self.corn_fields.retain(|_, val| {
            if val.state!=CornFieldDataState::Stale{return true;}
            if val.ranges.is_empty(){return false;}
            self.corn_buffer_manager.add_stale_range(&val.ranges);
            return false;
        });
    }
    /// ### Returns the total instances of corn that are currently loading
    /// - Loaded and Stale data is not included
    /// - If the corn field couldn't get an id, it is also not included
    pub fn get_loading_corn_count(&self) -> u32{
        self.corn_fields.iter().filter_map(|data| {
            if data.1.state != CornFieldDataState::Loading {return None;}
            return Some(data.1.get_instance_count() as u32);
        }).sum()
    }
    /// ### Sets the state of uninitialized data to loading
    pub fn init_new_data_load(&mut self){
        for (_, data) in self.corn_fields.iter_mut(){
            if data.state != CornFieldDataState::Uninitialized {continue;}
            data.state = CornFieldDataState::Loading;
        }
    }
    /// ### Assigns a list of ranges to new data
    /// - ranges are taken first from the stale ranges
    /// - each range corresponds to a range in either the expanded or normal buffer
    pub fn assign_new_data_ranges(&mut self){
        self.corn_fields.iter_mut().filter(|(_, v)| v.state==CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            data.ranges = self.corn_buffer_manager.get_ranges(data.get_instance_count() as u32);
        });
    }
    /// ### Returns whether or not the buffer is fragmented too much
    /// #### TODO: Get a metric for fragmentation and a limit here
    pub fn buffer_is_fragmented(&self) -> bool{return false;}
//==========================================================================================
    /// ### Queues a Buffer Expansion process during the render phase
    /// - Expands the buffer to (current size + count)*1.5
    /// - replaces the real instance buffer with an expanded one, so that we can expand and initialize on the same frame
    pub fn queue_expansion(&mut self, count: u32, render_device: &RenderDevice){
        self.corn_buffer_manager.queue_expansion(count, render_device);
    }
    /// ### Queues a data initialization and stale data flagging operation
    /// - Creates the constant buffers used in the init compute pass
    /// - creates the bind group for the ini compute pass
    pub fn queue_init(&mut self, render_device: &RenderDevice, pipeline: &CornDataPipeline){
        let new_data: Vec<(Vec<Range<u32>>, CornFieldSettings)> = self.corn_fields
            .iter()
            .filter_map(|(_, v)| {
                if v.state!=CornFieldDataState::Loading {return None;}
                if !v.ranges.is_empty(){
                    return Some((v.ranges.to_owned(), v.settings));
                }
                return None;
            }).collect();
        self.corn_buffer_manager.create_init_structures(new_data, render_device, pipeline);
    }
    /// ### Queues up a defrag then shrink operation
    /// - creates a shrunken buffer to store the defragmented data in
    /// - assigns each corn field a new range to use in the defraged buffer
    /// - creates the buffers and bind group for use in the defrag compute shader
    pub fn queue_buffer_shrink(&mut self, render_device: &RenderDevice, pipeline: &CornDataPipeline){
        self.corn_buffer_manager.queue_buffer_shrink(render_device);
        self.assign_defragmented_ranges();
        self.create_defrag_structures(render_device, pipeline);
    }
    /// ### Queues up just a defrag operation
    /// - creates a defragmented buffer to store the defragmented data in
    /// - assigns each corn field a new range to use in teh buffer
    /// - creates the buffers and bind group for use in teh defrag compute shader
    pub fn queue_buffer_defragmentation(&mut self, render_device: &RenderDevice, pipeline: &CornDataPipeline){
        self.corn_buffer_manager.queue_defragmentation(render_device);
        self.assign_defragmented_ranges();
        self.create_defrag_structures(render_device, pipeline);
    }
    /// ### Queues up a cpu readback operation
    /// - creates and maps a buffer used to copy the end result of a data pipeline pass back to the cpu
    pub fn queue_cpu_readback(&mut self, render_device: &RenderDevice){
        if !self.corn_buffer_manager.update_pending() {return;}
        self.corn_buffer_manager.queue_cpu_readback(render_device);
    }
//==========================================================================================
    /// ### Creates the structures used in the defragment compute pass
    pub fn create_defrag_structures(&mut self, render_device: &RenderDevice, pipeline: &CornDataPipeline){
        let id_range_offset: Vec<(Vec<Range<u32>>, u32)> = self.corn_fields
            .iter().filter_map(|(_, v)| {
                if v.state!=CornFieldDataState::Loading && v.state != CornFieldDataState::Loaded {return None;}
                if !v.ranges.is_empty(){
                    if let Some(new_range) = v.defragmented_range.as_ref(){
                        return Some((v.ranges.to_owned(), new_range.start));
                    }
                }
                return None;
            }).collect();
        self.corn_buffer_manager.create_defrag_structures(id_range_offset, render_device, pipeline);
    }
    /// ### Gives a range to each loaded or loading corn field from the defragmented buffers open spaces
    pub fn assign_defragmented_ranges(&mut self){
        self.corn_fields.iter_mut()
            .filter(|(_, v)| v.state == CornFieldDataState::Loaded || v.state == CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            data.defragmented_range = Some(self.corn_buffer_manager.get_defragmented_range(data.get_instance_count() as u32));
        });
    }
//==========================================================================================
    /// ### Updates all loading corn to be loaded
    pub fn finish_loading_corn(&mut self){
        self.corn_fields.iter_mut()
            .filter(|(_, v)|v.state==CornFieldDataState::Loading)
            .for_each(|(_, v)| 
        {
            v.state = CornFieldDataState::Loaded;
        });
    }
    /// ### Finishes defragmentation
    /// sets corn fields ranges to their defragmented versions
    pub fn finish_defragment(&mut self){
        self.corn_fields.iter_mut()
            .filter(|(_, v)| v.state == CornFieldDataState::Loaded)
            .for_each(|(_, data)| 
        {
            if let Some(ranges) = data.defragmented_range.as_ref(){
                data.ranges = vec![ranges.to_owned()];
                data.defragmented_range = None;
            }
        });
    }
    /// ### Erases all data from the struct
    pub fn destroy(&mut self){
        self.corn_fields = HashMap::new();
        self.displaced_stale_data = VecDeque::new();
        self.corn_buffer_manager.destroy();
        self.corn_buffer_manager = DynamicBufferManager::new();
    }
//==========================================================================================
}
/// Buffer Management Struct
#[derive(Default)]
pub struct DynamicBufferManager{
    /// Usually points to the same buffer as corn instance buffer resource, unless it needs to change size
    instance_buffer: Option<Buffer>,
    /// Holds the pointer to the corn instance buffer resource during buffer expansions
    original_instance_buffer: Option<Buffer>,
    /// list of all unused ranges in the instance buffer
    ranges: Vec<Range<u32>>,
    /// list of ranges that contain stale data
    stale_ranges: Vec<Range<u32>>,
    /// Represents the state of the actively planned actions
    planned_actions: BufferActions,
    /// current working size of the buffer: expanding: size of expanded buffer, shrinking: size of shrunken buffer
    active_size: u32,
    /// buffer where we place defragmented data
    defragmented_buffer: Option<Buffer>,
    /// list of ranges of the defragmented buffer.
    /// ranges are assigned to all data when we queue defragmentation
    /// realistically, this will only ever have 1 item since portions are only ever taken out, not readded in
    defragmented_ranges: Option<Vec<Range<u32>>>,
    /// the buffer we copy instance data to when we read back to the cpu
    readback_buffer: Option<Buffer>,
    /// holds compute shader structures necessary for dispatch such as bind groups and constant buffers
    compute_structures: ComputeStructures
}
impl DynamicBufferManager{
//==========================================================================================
    /// ### Returns a new empty struct
    pub fn new() -> Self{
        Self { 
            instance_buffer: None, 
            original_instance_buffer: None, 
            ranges: vec![], 
            stale_ranges: vec![], 
            planned_actions: BufferActions::NONE,
            active_size: 0, 
            defragmented_buffer: None,
            defragmented_ranges: None,
            readback_buffer: None, 
            compute_structures: ComputeStructures {
                settings_buffer: None, 
                defrag_ranges_buffer: None,
                init_bind_group: None, 
                defrag_bind_group: None,
                total_init_corn: 0, 
                total_defrag_corn: 0 
            } 
        }
    }
    /// ### Initializes the struct with a given instance buffer and size
    pub fn init(&mut self, instance_count: u32, instance_buffer: &Buffer){
        self.ranges = vec![(0..instance_count)];
        self.active_size = instance_count;
        self.instance_buffer = Some(instance_buffer.to_owned());
    }
    /// ### Resets per frame metadata such as planned actions and compute structures
    pub fn per_frame_reset(&mut self){
        self.planned_actions = BufferActions::NONE;
        self.compute_structures.destroy();
    }
    /// ### Erases all data from the struct
    pub fn destroy(&mut self){
        if let Some(buffer) = self.instance_buffer.as_ref(){buffer.destroy(); self.instance_buffer = None;}
        if let Some(buffer) = self.original_instance_buffer.as_ref(){buffer.destroy(); self.original_instance_buffer = None;}
        if let Some(buffer) = self.defragmented_buffer.as_ref(){buffer.destroy(); self.defragmented_buffer = None;}
        if let Some(buffer) = self.readback_buffer.as_ref(){buffer.destroy(); self.readback_buffer = None;}
        self.compute_structures.destroy();
        self.compute_structures = ComputeStructures{
            settings_buffer: None,
            defrag_ranges_buffer: None,
            init_bind_group: None,
            defrag_bind_group: None,
            total_defrag_corn:0,
            total_init_corn:0
        };
    }
//==========================================================================================
    /// ### Adds a range to the list of stale ranges
    pub fn add_stale_range(&mut self, stale_ranges: &Vec<Range<u32>>){
        self.stale_ranges.combine(stale_ranges);
    }
    /// ### Converts the stale ranges into regular ones
    pub fn convert_stale_ranges(&mut self){
        self.ranges.combine(&self.stale_ranges);
        self.stale_ranges = vec![];
    }
    /// ### Returns the total available space in the instance buffer
    /// - returns stale_ranges.total()+available_ranges.total()
    pub fn get_available_space(&self) -> u32{
        return self.ranges.total() + self.stale_ranges.total();
    }
    /// ### Removes and returns a list of ranges totaling count in length
    pub fn get_ranges(&mut self, count: u32) -> Vec<Range<u32>>{
        let (remaining, mut ranges) = self.stale_ranges.take(count);
        ranges.combine(&self.ranges.take(remaining).1);
        return ranges;
    }
    /// ### Returns whether or not the instance buffer is sparse
    /// - returns used space < total_space/3
    pub fn is_sparse(&self) -> bool{
        if self.planned_actions.contains(BufferActions::EXPAND) {return false;}
        self.ranges.total() + self.stale_ranges.total() > 2*self.active_size/3
    }
    /// ### Returns whether or not there is an operation pending
    /// - doesnt include cpu readback operations
    pub fn update_pending(&self) -> bool{
        self.planned_actions.intersects(BufferActions::ANY_CHANGE)
    }
    /// ### Removes and returns a range from the defragmented ranges
    pub fn get_defragmented_range(&mut self, count: u32) -> Range<u32>{
        // Assumes that we will only get one
        self.defragmented_ranges.as_mut().unwrap().take(count).1[0].to_owned()
    }
//==========================================================================================
    /// ### Queues up a buffer expansion
    /// - increases the buffer size to 1.5*(count+current_size)
    pub fn queue_expansion(&mut self, count: u32, render_device: &RenderDevice){
        self.planned_actions |= BufferActions::EXPAND;
        let original_size: u32 = self.active_size;
        self.active_size += count;
        self.active_size += self.active_size/2;
        self.original_instance_buffer = self.instance_buffer.clone();
        self.instance_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }));
        self.ranges.combine(&vec![original_size..self.active_size]);
    }
    /// ###  Queues up a buffer defragmentation
    /// - creates a new set of ranges corresponding to the defragmented buffer
    /// - creates the defragmentation structures
    pub fn queue_defragmentation(&mut self, render_device: &RenderDevice){
        self.planned_actions |= BufferActions::DEFRAGMENT;
        self.defragmented_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
            mapped_at_creation: false
        }));
        self.defragmented_ranges = Some(vec![0..self.active_size]);
    }
    /// ### Queues up a buffer shrink operation
    /// - shrinks the active size, then queues a defragmentation operation
    pub fn queue_buffer_shrink(&mut self, render_device: &RenderDevice){
        self.planned_actions |= BufferActions::SHRINK;
        self.active_size = self.active_size - self.stale_ranges.total() - self.ranges.total();
        self.active_size += self.active_size / 2;
        self.queue_defragmentation(render_device);
    }
    /// ### Queues up a CPU readback operation
    pub fn queue_cpu_readback(&mut self, render_device: &RenderDevice){
        self.planned_actions |= BufferActions::READBACK;
        self.readback_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Instance Readback Buffer"), 
            size: self.active_size as u64 * size_of::<PerCornData>() as u64, 
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
            mapped_at_creation: false
        }));
    }
//==========================================================================================
    /// ### Creates the buffers and bind groups used in the init compute pass
    pub fn create_init_structures(
        &mut self, 
        new_data: Vec<(Vec<Range<u32>>, CornFieldSettings)>, 
        render_device: &RenderDevice, 
        pipeline: &CornDataPipeline
    ){
        self.planned_actions |= BufferActions::INITIALIZE;
        self.compute_structures.create_init_structures(
            new_data,
            self.stale_ranges.to_owned(),
            self.instance_buffer.as_ref().unwrap(),
            render_device,
            pipeline
        );
    }
    /// ### Creates the buffers and bind groups used in the defrag compute pass
    pub fn create_defrag_structures(
        &mut self,
        id_range_offset: Vec<(Vec<Range<u32>>, u32)>,
        render_device: &RenderDevice,
        pipeline: &CornDataPipeline
    ){
        self.compute_structures.create_defrag_structures(
            id_range_offset, 
            self.instance_buffer.as_ref().unwrap(), 
            self.defragmented_buffer.as_ref().unwrap(), 
            render_device, 
            pipeline
        );
    }
//==========================================================================================
    /// ### Runs the compute pass
    /// - Expand
    /// - Init/Flag Stale
    /// - Defrag
    /// - Shrink
    /// - Readback
    pub fn run_compute_pass(&self, render_context: &mut RenderContext, world: &World){
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<CornDataPipeline>();
        if self.planned_actions.contains(BufferActions::EXPAND){
            render_context.command_encoder().copy_buffer_to_buffer(
                self.original_instance_buffer.as_ref().unwrap(), 
                0, 
                self.instance_buffer.as_ref().unwrap(), 
                0, 
                self.original_instance_buffer.as_ref().unwrap().size() as u64
            );
        }
        if self.planned_actions.contains(BufferActions::INITIALIZE){
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.init_id){
                let mut compute_pass = render_context.command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {label: Some("Initialize Corn Data Pass") });
                compute_pass.set_pipeline(&compute_pipeline);
                compute_pass.set_bind_group(0, self.compute_structures.init_bind_group.as_ref().unwrap(), &[]);
                compute_pass.dispatch_workgroups((self.compute_structures.total_init_corn as f32 / 256.0).ceil() as u32, 1, 1);
            }
        }
        if self.planned_actions.contains(BufferActions::DEFRAGMENT){
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.defrag_id){
                let mut compute_pass = render_context.command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {label: Some("Defrag Corn Data Pass") });
                compute_pass.set_pipeline(&compute_pipeline);
                compute_pass.set_bind_group(0, self.compute_structures.defrag_bind_group.as_ref().unwrap(), &[]);
                compute_pass.dispatch_workgroups((self.compute_structures.total_defrag_corn as f32 / 256.0).ceil() as u32, 1, 1);
            }
        }
        if self.planned_actions.contains(BufferActions::READBACK){
            render_context.command_encoder().copy_buffer_to_buffer(
                self.defragmented_buffer.as_ref().unwrap_or(self.instance_buffer.as_ref().unwrap()), 
                0, 
                self.readback_buffer.as_ref().unwrap(), 
                0, 
                self.readback_buffer.as_ref().unwrap().size() as u64
            );
        }
    }
//==========================================================================================
    /// ### Finishes the expansion operation, swapping the new buffer with the official one
    pub fn finish_expansion(&mut self, instance_buffer: &mut CornInstanceBuffer, render_device: &RenderDevice){
        instance_buffer.swap_data_buffers(
            self.instance_buffer.as_ref().unwrap(),
            self.active_size,
            render_device
        );
        self.original_instance_buffer = None;
    }
    /// ### Finishes the defragment operation
    /// - swaps the defragmented buffer with the official one
    /// - sets the ranges as our defragmented ranges
    pub fn finish_defragment(&mut self, instance_buffer: &mut CornInstanceBuffer, render_device: &RenderDevice){
        instance_buffer.swap_data_buffers(
            self.defragmented_buffer.as_ref().unwrap(),
            self.active_size,
            render_device
        );
        self.instance_buffer = self.defragmented_buffer.clone();
        self.defragmented_buffer = None;
        self.ranges = self.defragmented_ranges.as_ref().unwrap().to_owned();
        self.defragmented_ranges = None;
    }
    /// ### Finishes CPU Readback and prints results to console
    pub fn finish_cpu_readback(&mut self, render_device: &RenderDevice){
        let slice = self.readback_buffer.as_ref().unwrap().slice(..);
        let flag: Arc<Mutex<Box<bool>>> = Arc::new(Mutex::new(Box::new(false)));
        let flag_captured = flag.clone();
        slice.map_async(MapMode::Read, move |v|{
            let mut a = flag_captured.lock().unwrap();
            **a = v.is_ok().to_owned();
            drop(a);
            drop(v);
        });
        render_device.poll(Maintain::Wait);
        let a = flag.lock().unwrap();
        if **a {
            let raw = self.readback_buffer.as_ref().unwrap()
                .slice(..).get_mapped_range()
                .iter().map(|v| *v).collect::<Vec<u8>>();
            let data = bytemuck::cast_slice::<u8, PerCornData>(raw.as_slice()).to_vec();
            for corn in data{
                println!("{:?}", corn);
            }
            println!("");
        }
        self.readback_buffer.as_mut().unwrap().destroy();
        self.readback_buffer = None;
    }
//==========================================================================================
}
#[derive(Default)]
/// Stores per frame compute shader values
pub struct ComputeStructures{
    /// used in init, list of per corn field settings
    settings_buffer: Option<Buffer>,
    /// used in defrag, list of all data ranges
    defrag_ranges_buffer: Option<Buffer>,
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
    pub fn create_init_structures(
        &mut self, 
        new_data: Vec<(Vec<Range<u32>>, CornFieldSettings)>, 
        stale_data: Vec<Range<u32>>,
        instance_buffer: &Buffer,
        render_device: &RenderDevice,
        pipeline: &CornDataPipeline
    ){
        let settings: Vec<ComputeSettings> = new_data.iter().flat_map(|(ranges, settings)| {
            ranges.into_compute_ranges(false).into_iter().map(|range| 
                ComputeSettings::from((settings.to_owned(), range))
            )
        }).chain(stale_data.into_compute_ranges(true).into_iter().map(|stale_range| {
            ComputeSettings::from((CornFieldSettings::default(), stale_range.to_owned()))
        })).collect();
        self.total_init_corn = settings.iter().map(|r| r.range.length).sum();
        self.settings_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
            label: Some("Corn Settings Buffer"), 
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&settings[..])
        }));
        let init_bind_group = [
            BindGroupEntry{
                binding: 0,
                resource: BindingResource::Buffer(instance_buffer.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 1,
                resource: BindingResource::Buffer(self.settings_buffer.as_ref().unwrap().as_entire_buffer_binding())
            }    
        ];
        self.init_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Init Buffer Bind Group"), 
            layout: &pipeline.init_bind_group, 
            entries: &init_bind_group
        }));
    }
    pub fn create_defrag_structures(
        &mut self,
        id_range_offset: Vec<(Vec<Range<u32>>, u32)>,
        instance_buffer: &Buffer,
        defrag_buffer: &Buffer,
        render_device: &RenderDevice,
        pipeline: &CornDataPipeline
    ){
        let ranges: Vec<ComputeRange> = id_range_offset.iter().flat_map(|(ranges, new_offset)| {
            ranges.into_compute_ranges(false).into_iter().map(|range| {
                let mut x = range.to_owned();
                x.stale_range = *new_offset;
                return x;
            })
        }).collect();
        self.total_defrag_corn = ranges.iter().map(|range| range.length).sum();
        self.defrag_ranges_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
            label: Some("Corn Defrag Ranges Buffer"), 
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&ranges[..])
        }));
        let defrag_bind_group = [
            BindGroupEntry{
                binding: 0,
                resource: BindingResource::Buffer(self.defrag_ranges_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 1,
                resource: BindingResource::Buffer(defrag_buffer.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 2,
                resource: BindingResource::Buffer(instance_buffer.as_entire_buffer_binding())
            }
        ];
        self.defrag_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Defrag Bind Group"), 
            layout: &pipeline.defrag_bind_group, 
            entries: &defrag_bind_group
        }));
    }
    pub fn destroy(&mut self){
        if let Some(buffer) = self.settings_buffer.as_ref(){buffer.destroy();}
        if let Some(buffer) = self.defrag_ranges_buffer.as_ref(){buffer.destroy();}
    }
}
/*========================
    Random Functionality
  ========================*/
pub trait Ranges<T>{
    fn combine(&mut self, other: &Self);
    fn total(&self) -> T;
    fn take(&mut self, count: T) -> (T, Self);
    fn into_compute_ranges(&self, stale: bool) -> Vec<ComputeRange>;
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
    fn into_compute_ranges(&self, stale: bool) -> Vec<ComputeRange> {
        self.iter().scan(0, |acc, val| {
            let range = ComputeRange{
                start: val.start,
                length: val.end - val.start,
                offset: *acc,
                stale_range: stale as u32
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
    Initialized
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
/// Respresents a Range for use in a compute shader
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct ComputeRange {
    start: u32,
    length: u32,
    /// instance offset for corresponding corn field
    offset: u32,
    /// 0-1 whether or not this range is meant as a stale range
    stale_range: u32
}
/// Respresents corn field settings for use in a compute shader
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct ComputeSettings {
    range: ComputeRange,
    origin: Vec3,
    res_width: u32,
    height_width_min: Vec2,
    step: Vec2,
    random_settings: Vec4
}
impl From::<(CornFieldSettings, ComputeRange)> for ComputeSettings{
    fn from(value: (CornFieldSettings, ComputeRange)) -> Self {
        let mut output = Self { 
            range: value.1.to_owned(),
            origin: value.0.get_origin(),
            height_width_min: Vec2::new(value.0.height_range.y-value.0.height_range.x, value.0.height_range.x),
            step: value.0.get_step(),
            res_width: (value.0.get_resolution().0*2-1) as u32,
            random_settings: Vec4::new(value.0.get_random_offset_range(), 0.0, 0.0, 0.0)
         };
         if !output.step.x.is_finite() || output.step.x.is_nan(){
            output.origin.x = value.0.center.x;
            output.step.x = 0.0;
         }
         if !output.step.y.is_finite() || output.step.y.is_nan(){
            output.origin.y = value.0.center.y;
            output.step.y = 0.0;
         }
         return output;
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
    ranges: Vec<Range<u32>>,
    defragmented_range: Option<Range<u32>>
}
impl CornFieldData{
    pub fn new(center: Vec3, half_extents: Vec2, seperation_distance: f32, height_range: Vec2, rand_offset: f32) -> Self{
        Self { 
            settings: CornFieldSettings { center, half_extents, dist_between: seperation_distance, height_range, rand_offset_factor: rand_offset}, 
            state: CornFieldDataState::Uninitialized, 
            ranges: vec![],
            defragmented_range: None
        }
    }
    pub fn get_instance_count(&self) -> u64{
        self.settings.get_instance_count()
    }
}
/// per corn field configuration settings
#[derive(Clone, Copy, Debug, Default)]
pub struct CornFieldSettings{
    center: Vec3,
    half_extents: Vec2,
    dist_between: f32,
    height_range: Vec2,
    rand_offset_factor: f32
}
impl CornFieldSettings{
    pub fn get_resolution(&self) -> (u64, u64){
        let width = self.half_extents.x.max(self.half_extents.y)*2.0;
        let height = self.half_extents.x.min(self.half_extents.y)*2.0;
        //bigger of the two width resolutions
        let width_res = (width/self.dist_between) as u64+1;
        // total height resolution of both big and small rows
        let height_res = ((2f32*height)/(self.dist_between*3f32.sqrt())) as u64+1;
        (width_res, height_res)
    }
    pub fn get_instance_count(&self) -> u64{
        let (width_res, height_res) = self.get_resolution();
        width_res*(height_res-height_res/2)+(width_res-1)*(height_res/2)
    }
    pub fn get_origin(&self) -> Vec3{
        let (width_res, height_res) = self.get_resolution();
        let true_width = (width_res-1) as f32*self.dist_between;
        let true_height = (height_res-1) as f32*self.dist_between*3f32.sqrt()*0.5;
        return self.center - Vec3::new(true_width*0.5, 0.0, true_height*0.5);
    }
    pub fn get_step(&self) -> Vec2{
        Vec2::new(
            self.dist_between*0.5, 
            self.dist_between*3f32.sqrt()*0.5
        )
    }
    pub fn get_random_offset_range(&self) -> f32{
        return self.dist_between*self.rand_offset_factor;
    }
}
bitflags! {
    /// Flags used to represent the planned actions of the corn data pipeline in the current frame
    #[derive(Debug, Clone, Copy, Default)]
    pub struct BufferActions: u8{
        const NONE = 0u8;
        const EXPAND = 1u8<<0;
        const INITIALIZE = 1u8<<1;
        const DEFRAGMENT = 1u8<<2;
        const SHRINK = (1u8<<3) | Self::DEFRAGMENT.bits();
        const READBACK = 1u8<<4;
        const ANY_CHANGE = Self::EXPAND.bits() | Self::INITIALIZE.bits() | Self::SHRINK.bits();
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
                            ty: BufferBindingType::Storage { read_only: false }, 
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
        let init_shader = world
            .resource::<AssetServer>()
            .load("shaders/corn/init.wgsl");
        let defrag_shader = world
            .resource::<AssetServer>()
            .load("shaders/corn/defrag.wgsl");
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
/*==========
    Plugin
  ==========*/
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
                prepare_corn_data.in_set(RenderSet::Prepare).run_if(in_state(InstanceBufferState::Initialized))
            ))
        .world.get_resource_mut::<RenderGraph>().unwrap()
            .add_node("Corn Buffer Data Pipeline", CornDataPipelineNode{});
    }
    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CornDataPipeline>();
    }
}