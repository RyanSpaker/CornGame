use std::{any::type_name, marker::PhantomData, sync::{Arc, Mutex}};
use bevy::{
    prelude::*, 
    utils::hashbrown::HashMap, 
    render::{
        render_resource::*, 
        renderer::{RenderDevice, RenderContext},
        render_graph::{Node, RenderGraphContext, RenderGraph, RenderLabel}, RenderApp, Render, RenderSet, render_asset::RenderAssets
    }
};
use bytemuck::{Pod, Zeroable};
use wgpu::Maintain;
use super::{
    operation_manager::{CornBufferOperations, CornInitOp},
    super::{buffer::{BufferRange, CornInstanceBuffer, PerCornData, CORN_DATA_SIZE}, field::RenderableCornFieldID}
};

/// All functionality necessary for the operation executor to run buffer operations
pub trait IntoOperationResources: Component + Sized{
    /// This function is responsible for creating the necessary buffers for initialization of the field data on the gpu
    /// The Buffers returned will be re passed into the get_init_bindgroups function in the same order
    /// should return all buffers created for use in the init shaders
    fn get_init_buffers(
        data: CreateInitBufferStructures<Self>
    ) -> Vec<(Buffer, Option<RenderableCornFieldID>)>;
    /// This function is responsible for creating the necessary bindgroups and execution counts for initialization of the field data on the gpu
    /// If the field's init function can be batched with other fields of the same type, then it should return a single (BindGroup, u64)
    /// the u64 is the execution count of the shader, per bind group
    fn get_init_bindgroups(
        data: CreateInitBindgroupStructures<Self>
    ) -> Vec<(BindGroup, u64)>;
}
/// Struct that holds the inputs to RenderableCornField.get_init_buffers()
pub struct CreateInitBufferStructures<'a, T: IntoOperationResources>{
    pub fields: Vec<(&'a T, BufferRange)>,
    pub render_device: &'a RenderDevice
}
/// Struct that holds the inputs to RenderableCornField.get_init_bindgroups()
pub struct CreateInitBindgroupStructures<'a, T: IntoOperationResources>{
    pub fields: Vec<(&'a T, BufferRange)>,
    pub render_device: &'a RenderDevice,
    pub images: &'a RenderAssets<Image>,
    pub layout: &'a BindGroupLayout,
    pub operation_buffer: &'a Buffer,
    pub buffers: &'a Vec<(Buffer, Option<RenderableCornFieldID>)>
}

/// Adds all functionality necessary for pipeline creation
pub trait IntoCornPipeline: Component {
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
}


/*====================*
 *  Shader Resources  *
 *====================*/

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

/*====================*
 *  Resource Structs  *
 *====================*/

/// A System Set for prepare_init_buffers<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct PrepareInitBuffersSet;

/// A System Set for prepare_init_bindgroups<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct PrepareInitBindgroupsSet;

/// Struct used to hold defrag and stale operation resources
#[derive(Default)]
pub struct OperationSettings{
    /// buffer with settings used in the shader
    settings_buffer: Option<Buffer>,
    /// Bind group used by the shader
    bindgroup: Option<BindGroup>,
    /// Total number of executions needed, ciel(#/256) = execution groups
    execution_count: u64
}
impl OperationSettings{
    pub fn destroy(&mut self){
        self.settings_buffer.take().and_then(|buffer| Some(buffer.destroy()));
        self.bindgroup = None;
        self.execution_count = 0;
    }
}

/// A resource which creates and holds all of the GPU resources necessary to initialize and manage the corn instance buffer
#[derive(Default, Resource)]
pub struct CornOperationResources{
    /// Hashmap of corn field type to bind group and total corn/ shader executions, 
    /// Corn fields that can't be batched will have multiple elements, while those that can will only have 1
    init_bindgroups: HashMap<String, Vec<(BindGroup, u64)>>,
    /// Holds all buffers used during a specific initialization operation
    init_buffers: HashMap<String, Vec<(Buffer, Option<RenderableCornFieldID>)>>,
    /// Holds resources for a flag stale compute pass
    stale_resources: OperationSettings,
    /// Holds resources for a defrag compute pass
    defrag_resources: OperationSettings,
    /// Holds the new ranges for a defrag pass
    defrag_replaced_ranges: Vec<(RenderableCornFieldID, BufferRange)>,
    /// The temporary buffer used during shrinking, expanding, and defragmenting, which will replace the instance buffer at the end of the render frame
    temporary_buffer: Option<Buffer>,
    ///Stores the cpu readback buffer
    readback_buffer: Option<Buffer>,
    /// Whether or not the resources are good to go or not.
    enabled: bool
}
impl CornOperationResources{
    /// Runs during the prepare phase, creating the per frame buffers necessary to execute operations that dont change on a per field type basis
    pub fn prepare_common_buffers(
        mut resources: ResMut<CornOperationResources>,
        operations: Res<CornBufferOperations>,
        render_device: Res<RenderDevice>,
        instance_buffer: Res<CornInstanceBuffer>
    ){
        // return early if the resources failed to do something (meaning enabled was set to false)
        if !resources.enabled {return;}
        // Setup expansion, shrinkin, and defrag resources
        if operations.expansion > 0 || operations.defrag{
            resources.temporary_buffer = Some(render_device.create_buffer(&BufferDescriptor { 
                label: Some("Corn Instance Buffer".into()), 
                size: operations.get_new_buffer_count()*CORN_DATA_SIZE, 
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX, 
                mapped_at_creation: false 
            }));
            if operations.defrag{
                let current_ranges: Vec<(RenderableCornFieldID, BufferRange)> = instance_buffer.ranges.to_owned().into_iter().collect();
                let mut new_ranges: Vec<(RenderableCornFieldID, BufferRange)> = vec![];
                // Holds execution count after for loop
                let mut pointer: u64 = 0;
                for (id, old_range) in current_ranges.iter(){
                    new_ranges.push((id.to_owned(), BufferRange::simple(&pointer, &(pointer + old_range.len()))));
                    pointer += old_range.len();
                }
                resources.defrag_replaced_ranges = new_ranges.clone();
                resources.defrag_resources.execution_count = pointer;
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
                resources.defrag_resources.settings_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor{ 
                    label: Some("Corn Defrag Ranges Buffer"), 
                    usage: BufferUsages::STORAGE,
                    contents: bytemuck::cast_slice(&compute_ranges[..])
                }));
            }
        }
        // Setup flag_stale resource
        if !operations.post_init_state.stale_space.is_empty() {
            resources.stale_resources.execution_count = operations.post_init_state.stale_space.len();
            let mut compute_ranges: Vec<ComputeStaleRange> = vec![];
            for (start, end) in operations.post_init_state.stale_space.get_continuos_ranges(){
                compute_ranges.push(ComputeStaleRange{start: start as u32, length: (end-start) as u32, _padding: UVec2::default()});
            }
            resources.stale_resources.settings_buffer = Some(render_device.create_buffer_with_data(&BufferInitDescriptor { 
                label: Some("Corn Flag Stale Ranges Buffer".into()), 
                contents: bytemuck::cast_slice(&compute_ranges[..]), 
                usage: BufferUsages::STORAGE 
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
    /// Runs during the prepare phase, creating the per frame bind groups necessary to execute operations that dont change on a per field type basis
    pub fn prepare_common_bind_group(
        mut resources: ResMut<CornOperationResources>,
        pipelines: Res<CornOperationPipelines>,
        operations: Res<CornBufferOperations>,
        render_device: Res<RenderDevice>,
        instance_buffer: Res<CornInstanceBuffer>
    ){
        // return early if the resources failed to do something (meaning enabled was set to false)
        if !resources.enabled {return;}
        // Setup expansion, shrinkin, and defrag resources
        if operations.defrag{
            let defrag_bind_group = [
                BindGroupEntry{
                    binding: 0,
                    resource: BindingResource::Buffer(resources.defrag_resources.settings_buffer.as_ref().unwrap().as_entire_buffer_binding())
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
            resources.defrag_resources.bindgroup = Some(render_device.create_bind_group( 
                Some("Corn Defrag Bind Group"), 
                &pipelines.bindgroups.get(&"Defrag".to_string()).as_ref().unwrap(), 
                &defrag_bind_group
            ));
        }
        // Setup flag_stale resource
        if !operations.post_init_state.stale_space.is_empty() {
            let stale_bind_group = [
                BindGroupEntry{
                    binding: 0,
                    resource: BindingResource::Buffer(instance_buffer.get_instance_buffer().unwrap().as_entire_buffer_binding())
                },
                BindGroupEntry{
                    binding: 1,
                    resource: BindingResource::Buffer(resources.stale_resources.settings_buffer.as_ref().unwrap().as_entire_buffer_binding())
                },
            ];
            resources.stale_resources.bindgroup = Some(render_device.create_bind_group(
                Some("Corn Stale Bind Group".into()),
                &pipelines.bindgroups.get(&"Stale".to_string()).as_ref().unwrap(),
                &stale_bind_group
            ));
        }
    }
    /// Runs once for each corn field type, creates the buffers used by the initialization shader
    pub fn prepare_init_buffers<T: IntoOperationResources + IntoCornPipeline>(
        fields: Query<(&T, &CornInitOp)>,
        mut resources: ResMut<CornOperationResources>,
        pipelines: Res<CornOperationPipelines>,
        render_device: Res<RenderDevice>
    ){
        // if resources failed to do something (enabled=false) or there are no fields of type T, return early
        if !resources.enabled || fields.is_empty() {return;}
        // Make sure the pipeline for our Field has at least been queuedby the pipeline resource
        if !pipelines.pipeline_queued::<T>() {
            resources.enabled = false;
            return;
        }
        let buffers = T::get_init_buffers(CreateInitBufferStructures{
            fields: fields.into_iter().map(|(field, op)| (field, op.range.clone())).collect(), 
            render_device: render_device.as_ref()
        });
        resources.init_buffers.insert(type_name::<T>().to_string(), buffers);
    }
    /// Runs once for each type of corn field during the prepare phase. Creates the init resources needed for initialization
    /// Most of the logic is delegated to the corn field type, including creating the bind groups and settings buffers.
    pub fn prepare_init_bindgroup<T: IntoOperationResources + IntoCornPipeline>(
        fields: Query<(&T, &CornInitOp)>,
        mut resources: ResMut<CornOperationResources>,
        images: Res<RenderAssets<Image>>,
        operations: Res<CornBufferOperations>,
        pipelines: Res<CornOperationPipelines>,
        render_device: Res<RenderDevice>,
        instance_buffer: Res<CornInstanceBuffer>
    ){
        // if resources failed to do something (enabled=false) or there are no fields of type T, return early
        if !resources.enabled || fields.is_empty() {return;}
        // Make sure the pipeline for our Field has at least been queuedby the pipeline resource
        if !pipelines.pipeline_queued::<T>() {
            resources.enabled = false;
            return;
        }
        let operation_buffer = if operations.expansion == 0 {
            instance_buffer.get_instance_buffer().unwrap()
        }else {resources.temporary_buffer.as_ref().unwrap()};

        let layout = pipelines.get_layout::<T>().unwrap();

        let bindgroups = T::get_init_bindgroups( CreateInitBindgroupStructures{
            fields: fields.into_iter().map(|(field, op)| (field, op.range.clone())).collect(),
            render_device: render_device.as_ref(),
            images: images.as_ref(),
            layout,
            operation_buffer,
            buffers: resources.init_buffers.get(&type_name::<T>().to_string()).unwrap()
        });
        resources.init_bindgroups.insert(type_name::<T>().to_string(), bindgroups);
    }
    /// Runs after the render phase but before cleanup, sending events to the storage manager containing the changed buffer state
    pub fn send_buffer_alteration_events(
        node_success: Res<NodeSuccess>,
        resources: Res<CornOperationResources>,
        operations: Res<CornBufferOperations>,
        init_ops: Query<(&RenderableCornFieldID, &CornInitOp)>,
        mut instance_buffer: ResMut<CornInstanceBuffer>
    ){
        if !resources.enabled {return;}
        if !*node_success.into_inner().value.lock().unwrap() {return;}
        if operations.defrag{
            let mut end_of_data: u64 = 0;
            for (_, range) in resources.defrag_replaced_ranges.iter(){
                end_of_data = end_of_data.max(range.end().unwrap_or(0));
            }
            instance_buffer.defrag(resources.defrag_replaced_ranges.clone(), BufferRange::default(), BufferRange::simple(&end_of_data, &operations.get_new_buffer_count()));
            return;
        }
        if operations.expansion > 0{
            instance_buffer.expand_space(operations.expansion);
        }
        if !operations.post_init_state.stale_space.is_empty(){
            instance_buffer.delete_stale_range(operations.post_init_state.stale_space.clone());
        }
        for (id, op) in init_ops.iter(){
            instance_buffer.alloc_space(op.range.clone(), id.clone());
        }
    }
    /// Deletes per frame resources during the cleanup phase
    /// Also replaces the instance buffer with the temporary one if necessary
    pub fn cleanup(
        mut resources: ResMut<CornOperationResources>,
        mut instance_buffer: ResMut<CornInstanceBuffer>,
        render_device: Res<RenderDevice>
    ){
        resources.stale_resources.destroy();
        resources.defrag_resources.destroy();
        resources.defrag_replaced_ranges = vec![];
        resources.init_bindgroups.clear();
        resources.init_buffers.drain().for_each(|(_, buffers)| buffers.into_iter().for_each(|(buffer, _)| buffer.destroy()));

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
        
        if !resources.enabled{
            resources.temporary_buffer.take().and_then(|buffer| Some(buffer.destroy())); 
            resources.enabled = true;
            return;
        }

        resources.enabled = true;
        if let Some(buffer) = resources.temporary_buffer.take(){
            let new_size = buffer.size() / CORN_DATA_SIZE;
            instance_buffer.swap_instance_buffer(buffer, new_size, render_device.as_ref());
        }
    }
    /// Returns all system configuration for this functionality
    pub fn add_systems(app: &mut App) {
        app
        .configure_sets(Render, PrepareInitBuffersSet.in_set(RenderSet::PrepareResources))
        .configure_sets(Render, PrepareInitBindgroupsSet.in_set(RenderSet::PrepareBindGroups))
        .add_systems(Render, (
            Self::prepare_common_buffers.in_set(RenderSet::PrepareResources),
            Self::prepare_common_bind_group.in_set(RenderSet::PrepareBindGroups),
            (
                (Self::send_buffer_alteration_events, Self::cleanup).chain().before(CornInstanceBuffer::cleanup).before(CornInstanceBuffer::update_indirect_buffer),
                NodeSuccess::reset
            ).in_set(RenderSet::Cleanup)
        ));
    }
}

/*====================*
 *  Pipeline Structs  *
 *====================*/

/// A System Set for queue_init_pipeline_creation<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct QueuePipelineCreationSet;

/// Holds all pipelines used in the Data Pipeline node (Defrag, Stale, Initialization)
#[derive(Clone, Debug, Resource)]
pub struct CornOperationPipelines{
    /// Stores the pipelines using a string key, "Stale" -> stale pipeline, "Defrag" -> defrag pipeline, type_name(T) -> init pipeline for type T
    pub pipelines: HashMap<String, CachedComputePipelineId>,
    /// Stores the bind group layouts for each pipeline
    pub bindgroups: HashMap<String, BindGroupLayout>
}
impl FromWorld for CornOperationPipelines{
    /// Creates the pipelines with the defrag and stale pre queued
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let defrag_bind_group = render_device.create_bind_group_layout(
            Some("Defrag Corn Buffer Bind Group".into()),
            &[
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
            ]
        );
        let defrag_shader: Handle<Shader> = asset_server.load("shaders/corn/buffer_operation/defrag.wgsl");
        let defrag_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Defragment Corn Pipeline".into()),
            layout: vec![defrag_bind_group.clone()],
            push_constant_ranges: vec![],
            shader: defrag_shader,
            shader_defs: vec![],
            entry_point: "defragment".into(),
        });

        let stale_bind_group = render_device.create_bind_group_layout(
            Some("Flag Stale Corn Buffer Bind Group".into()),
            &[
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
        );
        let stale_shader: Handle<Shader> = asset_server.load("shaders/corn/buffer_operation/flag_stale.wgsl");
        let stale_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor{
            label: Some("Flag Stale Corn Pipeline".into()),
            layout: vec![stale_bind_group.clone()],
            push_constant_ranges: vec![],
            shader: stale_shader,
            shader_defs: vec![],
            entry_point: "flag_stale".into()
        });

        Self{
            pipelines: HashMap::from_iter([
                ("Defrag".to_string(), defrag_pipeline),
                ("Stale".to_string(), stale_pipeline)
            ].into_iter()),
            bindgroups: HashMap::from_iter([
                ("Defrag".to_string(), defrag_bind_group),
                ("Stale".to_string(), stale_bind_group)
            ].into_iter()),
        }
    }
}
impl CornOperationPipelines{
    /// Runs once for each type of corn field, queueing up the compute init pipeline and bind group layout
    pub fn queue_init_pipeline<T: IntoCornPipeline>(
        mut pipeline_res: ResMut<CornOperationPipelines>,
        pipeline_cache: Res<PipelineCache>,
        render_device: Res<RenderDevice>,
        asset_server: Res<AssetServer>
    ){
        let typename = type_name::<T>().to_string();
        if pipeline_res.pipelines.contains_key(&typename){
            if let CachedPipelineState::Err(PipelineCacheError::ShaderNotLoaded(_)) = pipeline_cache.get_compute_pipeline_state(pipeline_res.pipelines.get(&typename).unwrap().clone()){
                let descriptor = pipeline_cache.get_compute_pipeline_descriptor(pipeline_res.pipelines.get(&typename).unwrap().clone());
                let id = pipeline_cache.queue_compute_pipeline(descriptor.clone());
                pipeline_res.pipelines.insert(typename, id);
            }
            return;
        }
        let desc = T::init_bind_group_descriptor();
        let bind_group_layout = render_device.create_bind_group_layout(desc.label, desc.entries);
        let init_shader: Handle<Shader> = asset_server.load(T::init_shader());
        let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(("Initialize Corn Pipeline: ".to_string() + &typename).into()),
            layout: vec![bind_group_layout.clone()],
            push_constant_ranges: T::init_push_constant_ranges(),
            shader: init_shader.clone(),
            shader_defs: T::init_shader_defs(),
            entry_point: T::init_entry_point().into(),
        });
        pipeline_res.pipelines.insert(typename.clone(), id);
        pipeline_res.bindgroups.insert(typename, bind_group_layout);
    }
    /// Checks if a pipeline for the type T has been queued yet
    pub fn pipeline_queued<T: IntoCornPipeline>(&self) -> bool{
        self.pipelines.contains_key(type_name::<T>())
    }
    /// Returns bind group for a specific Corn Field Type
    pub fn get_layout<T: IntoCornPipeline>(&self) -> Option<&BindGroupLayout>{
        self.bindgroups.get(&type_name::<T>().to_string())
    }
    /// Returns the system configuration for this resource
    pub fn add_systems(app: &mut App) {
        // Pipeline creation is put in prepare assets, no real reason except to make sure they are created before our operation resources functions
        app.configure_sets(Render, QueuePipelineCreationSet.in_set(RenderSet::PrepareAssets));
    }
    /// Gets a pipeline using a str, if it exists
    pub fn get(&self, id: &str) -> Option<CachedComputePipelineId>{
        self.pipelines.get(&id.to_string()).cloned()
    }
    /// Gets a pipeline using a str, panicking if it doesnt exist
    pub fn get_unchecked(&self, id: &str) -> CachedComputePipelineId{
        self.pipelines.get(&id.to_string()).unwrap().clone()
    }
}

/*===============*
 *  Render Node  *
 *===============*/

/// This resource is used by the pipeline node to tell the world whether it was successful or not. 
/// Since we cant have mutable world access we have to do it with a mutex
#[derive(Resource, Default)]
pub struct NodeSuccess{
    value: Mutex<bool>
}
impl NodeSuccess{
    /// Called during cleanup, reset the value to assume the node is unsuccessful, it is overwritten by the node if otherwise
    pub fn reset(res: Res<Self>) {
        let mut value = res.value.lock().unwrap();
        if let Err(_) = value.set(Box::new(false)){
            panic!("Couldn't Reset Node Success Mutex!");
        }
    }
}

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, RenderLabel)]
pub struct CornBufferOperationsStage;

/// This is the render graph node which executes any buffer operation
pub struct CornBufferOperationsNode;
impl Node for CornBufferOperationsNode{
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        let resources = world.resource::<CornOperationResources>();
        if !resources.enabled {return Ok(());}

        let operations = world.resource::<CornBufferOperations>();
        let pipelines = world.resource::<CornOperationPipelines>();
        let node_success = world.resource::<NodeSuccess>();

        let pipeline_cache = world.resource::<PipelineCache>();
        // compile list of all needed pipelines
        let mut needed_pipeline_ids: HashMap<String, CachedComputePipelineId> = HashMap::new();
        if operations.defrag {needed_pipeline_ids.insert("Defrag".to_string(), pipelines.get_unchecked("Defrag"));}
        if !operations.post_init_state.stale_space.is_empty() {needed_pipeline_ids.insert("Stale".to_string(), pipelines.get_unchecked("Stale"));}
        let mut critical_error = false;
        // add needed init pipelines
        resources.init_bindgroups.iter().for_each(|(typename, bindgroups)| {
            if !bindgroups.is_empty(){
                if pipelines.pipelines.contains_key(typename){
                    needed_pipeline_ids.insert(typename.clone(), pipelines.pipelines.get(typename).unwrap().clone());
                }else{
                    critical_error = true;
                }
            }
        });
        
        // turn ids into actual pipelines
        let mut needed_pipelines = HashMap::new();
        needed_pipeline_ids.iter().for_each(|(typename, id)| {
            let pipeline = pipeline_cache.get_compute_pipeline(*id);
            if pipeline.is_none() {
                critical_error = true;
            }
            else {
                needed_pipelines.insert(typename.clone(), pipeline.unwrap());
            }
        });
        // If there was an error getting our pipelines, return early, not updating node_success to be true
        if critical_error {return Ok(());}
        // Otherwise, run the shader code, and update the node success resource to be true
        let mut mutex_guard = node_success.value.lock().unwrap();
        if let Err(_) = mutex_guard.set(Box::new(true)) {
            panic!("Couldn't Set Node Success Mutex to true!");
        }
        drop(mutex_guard);

        //Defrag/Shrink if we need to
        if operations.defrag{
            let defrag_pipeline = needed_pipelines.get(&"Defrag".to_string()).unwrap();
            let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{label: Some("Corn Defragment Pass".into()), timestamp_writes: None});
            compute_pass.set_pipeline(defrag_pipeline);
            compute_pass.set_bind_group(0, resources.defrag_resources.bindgroup.as_ref().unwrap(), &[]);
            compute_pass.dispatch_workgroups((resources.defrag_resources.execution_count as f32 / 256.0).ceil() as u32, 1, 1);
            //Readback if needed
            if resources.readback_buffer.is_some(){
                drop(compute_pass);
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
        if !operations.post_init_state.stale_space.is_empty(){
            let stale_pipeline = needed_pipelines.get(&"Stale".to_string()).unwrap();
            let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{label: Some("Corn Flag Stale Pass".into()), timestamp_writes: None});
            compute_pass.set_pipeline(stale_pipeline);
            compute_pass.set_bind_group(0, resources.stale_resources.bindgroup.as_ref().unwrap(), &[]);
            compute_pass.dispatch_workgroups((resources.stale_resources.execution_count as f32 / 256.0).ceil() as u32, 1, 1);
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
        if operations.init_count > 0{
            let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{label: Some("Corn Init Pass".into()), timestamp_writes: None});
            for (typename, bind_groups) in resources.init_bindgroups.iter(){
                let init_pipeline = needed_pipelines.get(typename).unwrap();
                compute_pass.set_pipeline(init_pipeline);
                for (bindgroup, total_corn) in bind_groups{
                    compute_pass.set_bind_group(0, &bindgroup, &[]);
                    compute_pass.dispatch_workgroups((*total_corn as f32 / 256.0).ceil() as u32, 1, 1);
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

/*===========*
 *  Plugins  *
 *===========*/

/// Adds the Operation Executor functionality to the game, including resource creation and the node
pub struct CornOperationExecutionPlugin;
impl Plugin for CornOperationExecutionPlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<CornOperationResources>()
            .init_resource::<NodeSuccess>();
        CornOperationResources::add_systems(app.sub_app_mut(RenderApp));
        CornOperationPipelines::add_systems(app.sub_app_mut(RenderApp));
    }
    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<CornOperationPipelines>()
            .world.resource_mut::<RenderGraph>()
                .add_node(CornBufferOperationsStage, CornBufferOperationsNode);
    }
}

/// Adds operation executor functionality to the game for each type of renderable corn field
pub struct CornFieldOperationExecutionPlugin<T: IntoOperationResources + IntoCornPipeline>{
    _marker: PhantomData<T>
}
impl<T: IntoOperationResources + IntoCornPipeline> CornFieldOperationExecutionPlugin<T>{
    pub fn new() -> Self {
        CornFieldOperationExecutionPlugin { _marker: PhantomData::<T> }
    }
}
impl<T: IntoOperationResources + IntoCornPipeline> Plugin for CornFieldOperationExecutionPlugin<T>{
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app.add_systems(Render, (
                CornOperationResources::prepare_init_bindgroup::<T>.in_set(PrepareInitBindgroupsSet),
                CornOperationResources::prepare_init_buffers::<T>.in_set(PrepareInitBuffersSet),
                CornOperationPipelines::queue_init_pipeline::<T>.in_set(QueuePipelineCreationSet)
            ));
        }
    }
}
