use std::{any::type_name, marker::PhantomData, sync::{Arc, Mutex}};
use bevy::{
    prelude::*, 
    utils::hashbrown::HashMap, 
    render::{
        render_resource::*, 
        renderer::{RenderDevice, RenderContext},
        render_graph::{Node, RenderGraphContext, RenderGraph}, RenderApp, Render, RenderSet
    }
};
use bytemuck::{Pod, Zeroable};
use wgpu::Maintain;
use crate::ecs::corn_field::{CORN_DATA_SIZE, PerCornData, CornInstanceBuffer, RenderableCornFieldID};
use super::{
    RenderableCornField, 
    operation_manager::CornBufferOperationCalculator, 
    storage_manager::{CornBufferStorageManager, BufferRange, DeleteStaleSpaceEvent, ExpandSpaceEvent, DefragEvent, AllocSpaceEvent}
};

/// This represents a range for use in a defrag shader
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct ComputeDefragRange {
    /// Start of the continuos data range in the old buffer
    start: u32,
    /// Length of the continuos data range in the old buffer
    length: u32,
    /// Offset of the instance index for this range in the original buffer
    instance_offset: u32,
    /// Offset of the corn field data in the new buffer. offset+new_offset is the offset of this specific range in the new buffer
    field_offset: u32,
}

/// This represents a range for use in a flag_stale shader
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct ComputeStaleRange {
    /// Start of the continuos data range in the buffer
    start: u32,
    /// Length of the continuos data range in the buffer
    length: u32,
    _padding: UVec2
}

/// A System Set for create_continous_init_operations<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct QueuePipelineCreationSet;

/// A System Set for create_continous_init_operations<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct PrepareInitResourcesSet;

/// A resource which creates and holds all of the GPU resources necessary to initialize and manage the corn instance buffer
#[derive(Default, Resource)]
pub struct CornOperationResources{
    /// Holds the pipelines used for initialization for each corn field type. Each type gets only 1 pipeline
    /// Also holds each corn field types bind group layout
    init_pipelines: HashMap<String, (BindGroupLayout, CachedComputePipelineId)>,
    /// Hashmap of corn field type to bind group and total corn/ shader executions, 
    /// Corn fields that can't be batched will have multiple elements, while those that can will only have 1
    init_bindgroups: HashMap<String, Vec<(BindGroup, u64)>>,
    /// Holds all buffers used during a specific initialization operation
    init_buffers: Vec<Buffer>,
    /// Holds the bind group layout and pipeline for the flag stale operation
    stale_pipeline: Option<(BindGroupLayout, CachedComputePipelineId)>,
    /// Holds the bind group for the flag stale operation
    stale_bindgroup: Option<BindGroup>,
    /// The settings buffer used in the flag stale shader
    stale_settings_buffer: Option<Buffer>,
    /// The total number of shader executions needed for the flag stale operation
    stale_operations: u64,
    /// The defrag bind group layout and pipeline id
    defrag_pipeline: Option<(BindGroupLayout, CachedComputePipelineId)>,
    /// The defrag operations bind group
    defrag_bindgroup: Option<BindGroup>,
    /// The new ranges each corn field will have after the defrag operation. Sent to the storage manager after the operation is completed
    new_defrag_ranges: Vec<(RenderableCornFieldID, BufferRange)>,
    /// The settings buffer used in the defrag shader
    defrag_settings_buffer: Option<Buffer>,
    /// The total number of shader executions used by the defrag operation
    defrag_operations: u64,
    /// The temporary buffer used during shrinking, expanding, and defragmenting, which will replace the instance buffer at the end of the render frame
    temporary_buffer: Option<Buffer>,
    /// stores whether or not he pipelines have been created. stuff only happens if yes
    pipelines_ready: bool,
    ///Stores the cpu readback buffer
    readback_buffer: Option<Buffer>
}
impl CornOperationResources{
    /// Runs once for each type of renderable corn field, queuing their pipelines for creation
    pub fn queue_pipeline_creation<T: RenderableCornField>(
        mut resources: ResMut<CornOperationResources>,
        pipeline_cache: Res<PipelineCache>,
        render_device: Res<RenderDevice>,
        asset_server: Res<AssetServer>
    ){
        let bind_group_layout = render_device.create_bind_group_layout(&T::init_bind_group_descriptor());
        let typename = type_name::<T>().to_string();
        if resources.init_pipelines.get(&typename).is_none(){
            let init_shader: Handle<Shader> = asset_server.load(T::init_shader());
            let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(("Initialize Corn Pipeline: ".to_string() + &typename).into()),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: T::init_push_constant_ranges(),
                shader: init_shader,
                shader_defs: T::init_shader_defs(),
                entry_point: T::init_entry_point().into(),
            });
            resources.init_pipelines.insert(typename, (bind_group_layout, id));
        }
    }
    /// Runs once to create the defragment and flag_stale pipeline for the corn buffer.
    pub fn create_defrag_and_stale_resources(
        mut resource: ResMut<CornOperationResources>,
        pipeline_cache: Res<PipelineCache>,
        render_device: Res<RenderDevice>,
        asset_server: Res<AssetServer>
    ){
        let defrag_bind_group = render_device.create_bind_group_layout(
            &BindGroupLayoutDescriptor {
                label: Some("Defrag Corn Buffer Bind Group".into()),
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
        let defrag_shader: Handle<Shader> = asset_server.load("shaders/corn/defrag.wgsl");
        let defrag_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Defragment Corn Pipeline".into()),
            layout: vec![defrag_bind_group.clone()],
            push_constant_ranges: vec![],
            shader: defrag_shader,
            shader_defs: vec![],
            entry_point: "defragment".into(),
        });
        resource.defrag_pipeline = Some((defrag_bind_group, defrag_pipeline));

        let stale_bind_group = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor{
            label: Some("Flag Stale Corn Buffer Bind Group".into()),
            entries: &[
                BindGroupLayoutEntry{
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { 
                        ty: BufferBindingType::Storage { read_only: false }, 
                        has_dynamic_offset: false, 
                        min_binding_size: None },
                    count: None
                },
                BindGroupLayoutEntry{
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer { 
                        ty: BufferBindingType::Storage { read_only: true }, 
                        has_dynamic_offset: false, 
                        min_binding_size: None },
                    count: None
                }
            ]
        });
        let stale_shader: Handle<Shader> = asset_server.load("shaders/corn/flag_stale.wgsl");
        let stale_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor{
            label: Some("Flag Stale Corn Pipeline".into()),
            layout: vec![stale_bind_group.clone()],
            push_constant_ranges: vec![],
            shader: stale_shader,
            shader_defs: vec![],
            entry_point: "flag_stale".into()
        });
        resource.stale_pipeline = Some((stale_bind_group, stale_pipeline));
    }
    /// Runs during the prepare phase, creating the per frame resources necessary to execute operations that dont change on a per field type basis
    pub fn prepare_common_resources(
        mut resources: ResMut<CornOperationResources>,
        operations: Res<CornBufferOperationCalculator>,
        render_device: Res<RenderDevice>,
        instance_buffer: Res<CornInstanceBuffer>,
        storage: Res<CornBufferStorageManager>,
        pipeline_cache: Res<PipelineCache>
    ){
        if !resources.set_pipeline_state(&pipeline_cache.as_ref()) {return;}
        // Setup expansion, shrinkin, and defrag resources
        if operations.expansion > 0 || operations.defrag{
            resources.temporary_buffer = Some(render_device.create_buffer(&BufferDescriptor { 
                label: Some("Corn Instance Buffer".into()), 
                size: operations.get_new_buffer_count()*CORN_DATA_SIZE, 
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX, 
                mapped_at_creation: false 
            }));
            if operations.defrag{
                let current_ranges: Vec<(RenderableCornFieldID, BufferRange)> = storage.ranges.to_owned().into_iter().collect();
                let mut new_ranges: Vec<(RenderableCornFieldID, BufferRange)> = vec![];
                let mut pointer: u64 = 0;
                for (id, old_range) in current_ranges.iter(){
                    new_ranges.push((id.to_owned(), BufferRange::simple(&pointer, &(pointer + old_range.len()))));
                    pointer += old_range.len();
                }
                resources.new_defrag_ranges = new_ranges.to_owned();
                resources.defrag_operations = pointer;
                let mut compute_ranges: Vec<ComputeDefragRange> = vec![];
                for ((_, old_range), (_, new_range)) in current_ranges.iter().zip(new_ranges.iter()){
                    let mut instance_counter: u64 = 0;
                    for (start, end) in old_range.get_continuos_ranges(){
                        let range = ComputeDefragRange{
                            start: start as u32, 
                            length: (end-start) as u32, 
                            instance_offset: instance_counter as u32, 
                            field_offset: new_range.min().unwrap() as u32
                        };
                        compute_ranges.push(range);
                        instance_counter += end-start;
                    }
                }
                resources.defrag_settings_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
                    label: Some("Corn Defrag Ranges Buffer"), 
                    usage: BufferUsages::STORAGE,
                    contents: bytemuck::cast_slice(&compute_ranges[..])
                }));
                let defrag_bind_group = [
                    BindGroupEntry{
                        binding: 0,
                        resource: BindingResource::Buffer(resources.defrag_settings_buffer.as_ref().unwrap().as_entire_buffer_binding())
                    },
                    BindGroupEntry{
                        binding: 1,
                        resource: BindingResource::Buffer(resources.temporary_buffer.as_ref().unwrap().as_entire_buffer_binding())
                    },
                    BindGroupEntry{
                        binding: 2,
                        resource: BindingResource::Buffer(instance_buffer.get_instance_buffer().unwrap().as_entire_buffer_binding())
                    }
                ];
                resources.defrag_bindgroup = Some(render_device.create_bind_group(&BindGroupDescriptor { 
                    label: Some("Corn Defrag Bind Group"), 
                    layout: &resources.defrag_pipeline.as_ref().unwrap().0, 
                    entries: &defrag_bind_group
                }));
            }
        }
        // Setup flag_stale resource
        if !operations.new_stale_space.is_empty() {
            resources.stale_operations = operations.new_stale_space.len();
            let mut compute_ranges: Vec<ComputeStaleRange> = vec![];
            for (start, end) in operations.new_stale_space.get_continuos_ranges(){
                compute_ranges.push(ComputeStaleRange{start: start as u32, length: (end-start) as u32, _padding: UVec2::default()});
            }
            resources.stale_settings_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor { 
                label: Some("Corn Flag Stale Ranges Buffer".into()), 
                contents: bytemuck::cast_slice(&compute_ranges[..]), 
                usage: BufferUsages::STORAGE 
            }));
            let stale_bind_group = [
                BindGroupEntry{
                    binding: 0,
                    resource: BindingResource::Buffer(instance_buffer.get_instance_buffer().unwrap().as_entire_buffer_binding())
                },
                BindGroupEntry{
                    binding: 1,
                    resource: BindingResource::Buffer(resources.stale_settings_buffer.as_ref().unwrap().as_entire_buffer_binding())
                },
            ];
            resources.stale_bindgroup = Some(render_device.create_bind_group(&BindGroupDescriptor{
                label: Some("Corn Stale Bind Group".into()),
                layout: &resources.stale_pipeline.as_ref().unwrap().0,
                entries: &stale_bind_group
            }));
        }
        // Create readback Buffer is any of the operations are happening
        if operations.readback{
            resources.readback_buffer = Some(render_device.create_buffer(&BufferDescriptor { 
                label: Some("Corn Readback Buffer".into()), 
                size: operations.get_new_buffer_count()*CORN_DATA_SIZE, 
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                mapped_at_creation: false
            }));
        }
    }
    /// Runs once for each type of corn field during the prepare phase. Creates the init resources needed for initialization
    /// Most of the logic is delegated to the corn field type, including creating the bind groups and settings buffers.
    pub fn prepare_init_resources<T: RenderableCornField>(
        all_fields: Query<&T>,
        mut resources: ResMut<CornOperationResources>,
        operations: Res<CornBufferOperationCalculator>,
        render_device: Res<RenderDevice>,
        instance_buffer: Res<CornInstanceBuffer>,
        cache: Res<PipelineCache>
    ){
        let fields: Vec<(&T, BufferRange, RenderableCornFieldID, String)> = all_fields.iter().filter_map(|field| {
            let id = field.gen_id();
            if let Some((range, typename)) = operations.init_ops.get(&id){
                return Some((field, range.to_owned(), id, typename.to_owned()));
            }else{
                return None;
            }
        }).collect();
        if fields.is_empty() || !resources.set_field_pipeline_state(cache.as_ref(), type_name::<T>().to_string()) {
            return;
        }
        let operation_buffer = if operations.expansion == 0 {
            assert!(instance_buffer.get_instance_buffer().is_some(), "No Instance Buffer{:?} \n {:?}", fields[0], operations);
            instance_buffer.get_instance_buffer().unwrap()
        }else {resources.temporary_buffer.as_ref().unwrap()};
        let (bindgroups, buffers) = T::get_init_resources(
            fields, 
            render_device.as_ref(),
            &resources.init_pipelines.get(&type_name::<T>().to_string()).unwrap().0, 
            operation_buffer
        );
        resources.init_bindgroups.insert(type_name::<T>().to_string(), bindgroups);
        resources.init_buffers.extend(buffers.into_iter());
    }
    /// Runs after the render phase but before cleanup, sending events to the storage manager containing the changed buffer state
    pub fn send_buffer_alteration_events(
        mut delete_space_event: EventWriter<DeleteStaleSpaceEvent>,
        mut expand_space_event: EventWriter<ExpandSpaceEvent>,
        mut defrag_space_event: EventWriter<DefragEvent>,
        mut alloc_space_event: EventWriter<AllocSpaceEvent>,
        resources: Res<CornOperationResources>,
        operations: Res<CornBufferOperationCalculator>
    ){
        if !resources.pipelines_ready {return;}
        if operations.defrag{
            let mut end_of_data: u64 = 0;
            for (_, range) in resources.new_defrag_ranges.iter(){
                end_of_data = end_of_data.max(range.end().unwrap_or(0));
            }
            defrag_space_event.send(DefragEvent { 
                ranges: resources.new_defrag_ranges.to_owned(), 
                stale_range: BufferRange::default(), 
                free_space: BufferRange::simple(&end_of_data, &operations.get_new_buffer_count())
            });
            return;
        }
        if operations.expansion > 0{
            expand_space_event.send(ExpandSpaceEvent { length: operations.expansion });
        }
        if !operations.new_stale_space.is_empty(){
            delete_space_event.send(DeleteStaleSpaceEvent { range: operations.new_stale_space.to_owned() });
        }
        if !operations.init_ops.is_empty(){
            for (id, (range, _)) in operations.init_ops.iter(){
                alloc_space_event.send(AllocSpaceEvent { field: id.to_owned(), range: range.to_owned() });
            }
        }
    }
    /// Deletes per frame resources during the cleanup phase
    /// Also replaces the instance buffer with the temporary one if necessary
    pub fn cleanup(
        mut resources: ResMut<CornOperationResources>,
        mut instance_buffer: ResMut<CornInstanceBuffer>,
        render_device: Res<RenderDevice>
    ){
        resources.stale_bindgroup = None;
        resources.stale_operations = 0;
        resources.stale_settings_buffer.take().and_then(|buffer| Some(buffer.destroy()));

        resources.defrag_bindgroup = None;
        resources.new_defrag_ranges = vec![];
        resources.defrag_operations = 0;
        resources.defrag_settings_buffer.take().and_then(|buffer| Some(buffer.destroy()));

        resources.init_bindgroups.clear();
        resources.init_buffers.drain(..).for_each(|buffer| buffer.destroy());

        if let Some(readback_buffer) = resources.readback_buffer.take(){
            let slice = readback_buffer.slice(..);
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
                let raw = readback_buffer
                    .slice(..).get_mapped_range()
                    .iter().map(|v| *v).collect::<Vec<u8>>();
                let data = bytemuck::cast_slice::<u8, PerCornData>(raw.as_slice()).to_vec();
                for corn in data{
                    println!("{:?}", corn);
                }
                println!("");
            }
            readback_buffer.destroy();
        }
        
        if !resources.pipelines_ready{resources.temporary_buffer.take().and_then(|buffer| Some(buffer.destroy())); return;}

        if let Some(buffer) = resources.temporary_buffer.take(){
            let new_size = buffer.size() / CORN_DATA_SIZE;
            instance_buffer.swap_instance_buffer(buffer, new_size, render_device.as_ref());
        }
    }
    /// Sets and returns whether or not the pipelines needed have been created. This includes only the defrag and stale pipeline
    pub fn set_pipeline_state(&mut self, cache: &PipelineCache) -> bool{
        if self.defrag_pipeline.is_some() && match cache.get_compute_pipeline_state(self.defrag_pipeline.as_ref().unwrap().1) {
            CachedPipelineState::Ok(_) => true, 
            CachedPipelineState::Err(_) => false, 
            CachedPipelineState::Queued => false
        } {
            if self.stale_pipeline.is_some() && match cache.get_compute_pipeline_state(self.stale_pipeline.as_ref().unwrap().1) {
                CachedPipelineState::Ok(_) => true, 
                CachedPipelineState::Err(_) => false, 
                CachedPipelineState::Queued => false
            } {
                self.pipelines_ready = true;
                return true;
            }
        }
        self.pipelines_ready = false;
        return false;
    }
    /// Assuming self.pipelines_ready is already set for stale and defrag pipelines, updates its value by making sure the pipeline for typename is loaded, returning the end value
    pub fn set_field_pipeline_state(&mut self, cache: &PipelineCache, typename: String) -> bool{
        if self.pipelines_ready && self.init_pipelines.get(&typename).is_some() && 
        match cache.get_compute_pipeline_state(self.init_pipelines.get(&typename).as_ref().unwrap().1) {
            CachedPipelineState::Ok(_) => true, 
            CachedPipelineState::Err(_) => false, 
            CachedPipelineState::Queued => false
        } {
            return true;
        }
        self.pipelines_ready = false;
        return false;
    }
}
/// This is the render graph node which executes any buffer operation
pub struct CornBufferOperationsNode{}
impl Node for CornBufferOperationsNode{
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        let operations = world.resource::<CornBufferOperationCalculator>();
        let resources = world.resource::<CornOperationResources>();
        if !resources.pipelines_ready {return Ok(());}
        let pipeline_cache = world.resource::<PipelineCache>();
        //Defrag/Shrink if we need to
        if operations.defrag{
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(resources.defrag_pipeline.as_ref().unwrap().1){
                let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{label: Some("Corn Defragment Pass".into())});
                compute_pass.set_pipeline(&compute_pipeline);
                compute_pass.set_bind_group(0, resources.defrag_bindgroup.as_ref().unwrap(), &[]);
                compute_pass.dispatch_workgroups((resources.defrag_operations as f32 / 256.0).ceil() as u32, 1, 1);
            }
            //Readback if needed
            if resources.readback_buffer.is_some(){
                let operation_buffer = resources.temporary_buffer.as_ref().unwrap();
                render_context.command_encoder().copy_buffer_to_buffer(
                    &operation_buffer, 
                    0, 
                    resources.readback_buffer.as_ref().unwrap(), 
                    0, 
                    operation_buffer.size() as u64
                );
            }
            return Ok(());
        }
        //Flag stale data if needed
        if !operations.new_stale_space.is_empty(){
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(resources.stale_pipeline.as_ref().unwrap().1){
                let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{label: Some("Corn Flag Stale Pass".into())});
                compute_pass.set_pipeline(&compute_pipeline);
                compute_pass.set_bind_group(0, resources.stale_bindgroup.as_ref().unwrap(), &[]);
                compute_pass.dispatch_workgroups((resources.stale_operations as f32 / 256.0).ceil() as u32, 1, 1);
            }
        }
        let instance_buffer = world.resource::<CornInstanceBuffer>().get_instance_buffer();
        //Expand if needed
        if operations.expansion > 0 && instance_buffer.is_some(){
            render_context.command_encoder().copy_buffer_to_buffer(
                &instance_buffer.unwrap(), 
                0, 
                resources.temporary_buffer.as_ref().unwrap(), 
                0, 
                instance_buffer.unwrap().size() as u64
            );
        }
        // Init if needed
        if !operations.init_ops.is_empty(){
            let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{label: Some("Corn Init Pass".into())});
            for (typename, bind_groups) in resources.init_bindgroups.iter(){
                if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(resources.init_pipelines.get(typename).unwrap().1){
                    compute_pass.set_pipeline(&compute_pipeline);
                    for (bindgroup, total_corn) in bind_groups{
                        compute_pass.set_bind_group(0, &bindgroup, &[]);
                        compute_pass.dispatch_workgroups((*total_corn as f32 / 256.0).ceil() as u32, 1, 1);
                    }
                }
            }
        }
        //Read back if needed
        if resources.readback_buffer.is_some(){
            let operation_buffer = if resources.temporary_buffer.is_some() {
                resources.temporary_buffer.as_ref().unwrap()
            } else {
                instance_buffer.unwrap()
            };
            render_context.command_encoder().copy_buffer_to_buffer(
                &operation_buffer, 
                0, 
                resources.readback_buffer.as_ref().unwrap(), 
                0, 
                operation_buffer.size() as u64
            );
        }
        return Ok(());
    }
}

/// Adds the Operation Executor functionality to the game, including resource creation and the node
pub struct MasterCornOperationExecutionPlugin;
impl Plugin for MasterCornOperationExecutionPlugin{
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            // pipeline creation functions must run after other prepare functions to ensure that we dont attempt to query a pipelines state before render, which creates the state
            render_app
                .init_resource::<CornOperationResources>()
                .add_systems(Render, (
                    CornOperationResources::send_buffer_alteration_events.after(RenderSet::Render).before(RenderSet::Cleanup),
                    CornOperationResources::prepare_common_resources.after(CornBufferOperationCalculator::finalize_operations).in_set(RenderSet::Prepare),
                    CornOperationResources::create_defrag_and_stale_resources
                        .after(CornOperationResources::prepare_common_resources)
                        .after(PrepareInitResourcesSet)
                        .run_if(run_once()).in_set(RenderSet::Prepare),
                    CornOperationResources::cleanup.in_set(RenderSet::Cleanup)
                ))
                .world.get_resource_mut::<RenderGraph>().unwrap()
                    .add_node("Corn Buffer Data Pipeline", CornBufferOperationsNode{});
            if let Some(schedule) = render_app.get_schedule_mut(Render){
                schedule
                    .configure_set(QueuePipelineCreationSet
                        .after(PrepareInitResourcesSet).after(CornOperationResources::prepare_common_resources)
                        .run_if(run_once()).in_set(RenderSet::Prepare))
                    .configure_set(PrepareInitResourcesSet.after(CornOperationResources::prepare_common_resources).in_set(RenderSet::Prepare));
            }
        }
    }
}

/// Adds operation executor functionality to the game for each type of renderable corn field
pub struct CornOperationExecutionPlugin<T: RenderableCornField>{
    _marker: PhantomData<T>
}
impl<T: RenderableCornField> CornOperationExecutionPlugin<T>{
    pub fn new() -> Self {
        CornOperationExecutionPlugin { _marker: PhantomData::<T> }
    }
}
impl<T: RenderableCornField> Plugin for CornOperationExecutionPlugin<T>{
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app.add_systems(Render, (
                CornOperationResources::queue_pipeline_creation::<T>.in_set(QueuePipelineCreationSet),
                CornOperationResources::prepare_init_resources::<T>.in_set(PrepareInitResourcesSet)
            ));
        }
    }
}

