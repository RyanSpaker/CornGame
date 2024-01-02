use std::sync::{Arc, Mutex};
use bevy::{
    prelude::*, 
    render::{
        render_resource::*, 
        renderer::{RenderDevice, RenderContext, RenderQueue}, 
        RenderApp, 
        RenderSet, 
        Render, 
        render_graph::{Node, RenderGraphContext, RenderGraph}, view::ExtractedView, Extract
    }, pbr::draw_3d_graph::node::SHADOW_PASS, core_pipeline::core_3d, utils::hashbrown::HashMap
};
use bytemuck::{Pod, Zeroable};
use wgpu::{Maintain, QuerySetDescriptor, QuerySet};
use crate::{ecs::{main_camera::MainCamera, corn_field::PerCornData}, prelude::corn_model::CornMeshes};
use super::{CornInstanceBuffer, CORN_DATA_SIZE};

const READBACK_ENABLED: bool = false;
const TIMING_ENABLED: bool = false;

/// Respresents frustum structure in compute shader sans lod distance cutoffs
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct FrustumValues {
    pub mat: Mat4,
    pub offset: Vec4
}
impl From<&ExtractedView> for FrustumValues{
    fn from(value: &ExtractedView) -> Self {
        Self{
            mat: value.projection*value.transform.compute_matrix().inverse(),
            offset: value.transform.translation().extend(1.0)
        }
    }
}

/*
    Resources: 
*/

/// Main app resource containing the distance cutoffs for each corn LOD
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct LodCutoffs(pub Vec<f32>);
impl LodCutoffs{
    //Makes sure lodcutoffs has the right number of lods
    pub fn update_lod_cutoffs(mut lods: ResMut<LodCutoffs>, corn_mesh: Res<CornMeshes>){
        if corn_mesh.loaded && corn_mesh.lod_count as usize != lods.0.len(){
            lods.0.resize(corn_mesh.lod_count as usize, 0.0);
        }
    }
    /// Runs during extract to copy lod cutoff data to the renderapp
    pub fn extract_lod_cutoffs(
        mut render_lods: ResMut<LodCutoffs>, 
        main_lods: Extract<Res<LodCutoffs>>,
    ){
        render_lods.0 = main_lods.0.clone();
    }
    // max_cutoff / step count
    // step count is a geomtetric series where the first lod has 1, seocnd has 1*k, third 1*k*k
    // the total steps equatest to (k^lod_count-1)/(k-1)
    pub fn new_geometric(count: u32, k: f32, max_cutoff: f32) -> Self{
        let step_size: f32 = max_cutoff/((k.powi(count as i32)-1.0)/(k-1.0));
        let lod_cutoffs = (0..count).map(|i| 
            (((k.powi((i+1) as i32)-1.0)/(k-1.0))*step_size).powi(2)
        ).collect();
        return Self(lod_cutoffs);
    }
}
/// ### Keeps hold of all of the vote-scan-compact shader resources
#[derive(Resource, Default)]
pub struct ScanPrepassResources{
    /// Total number of corn instances in the buffers
    instance_count: u64,
    /// Total number of corn lods to seperate by
    lod_count: u32,
    /// The id correlating to the instance buffer, used to know if the buffer has changed since we last checked
    buffer_id: u64,
    /// Buffer to hold the vote data for our corn instances
    vote_buffer: Option<Buffer>,
    /// Buffer to hold summation data for the two extra scan functions
    sum_buffers: Option<(Buffer, Buffer)>,
    /// Lengths of the sum buffers in instance count
    sum_sizes: (u64, u64),
    /// Bind group for the prepass shaders
    bind_group: Option<BindGroup>,
    /// Frustum values for the camera for this frame
    frustum_values: FrustumValues,
    /// Whether or not the prepass should run
    enabled: bool,
    /// Buffers used to read back buffer data to cpu for debug purposes
    readback_buffers: Option<(Buffer, Buffer, Buffer, Buffer, Buffer, Buffer)>,
    /// Query set to hold the timing information for the scan prepass
    timing_query_set: Option<QuerySet>,
    /// Buffer to hold the timing values of the scan prepass if timing is enabled
    timing_buffer: Option<(Buffer, Buffer)>
}
impl ScanPrepassResources{
    /// Runs during prepare phase, creates all buffers necessary for this frame
    pub fn prepare_buffers(
        mut resources: ResMut<ScanPrepassResources>, 
        instance_buffer: Res<CornInstanceBuffer>,
        render_device: Res<RenderDevice>,
        mut pipeline: ResMut<CornBufferPrePassPipeline>,
        pipeline_cache: Res<PipelineCache>,
        camera: Query<&ExtractedView, With<MainCamera>>
    ){
        resources.enabled = instance_buffer.enabled;
        if !instance_buffer.enabled {return;}
        resources.frustum_values = camera.single().into();

        if pipeline.ids.get(&instance_buffer.lod_count).is_none(){
            let mut descriptors = pipeline.get_pipeline_descriptors(instance_buffer.lod_count).to_vec(); descriptors.reverse();
            let ids = [
                pipeline_cache.queue_compute_pipeline(descriptors.pop().unwrap()), 
                pipeline_cache.queue_compute_pipeline(descriptors.pop().unwrap()), 
                pipeline_cache.queue_compute_pipeline(descriptors.pop().unwrap()), 
                pipeline_cache.queue_compute_pipeline(descriptors.pop().unwrap())
            ];
            pipeline.ids.insert(
                instance_buffer.lod_count, 
                ids
            );
        }
        
        if instance_buffer.get_instance_count() != resources.instance_count{
            resources.instance_count = instance_buffer.get_instance_count();
            resources.lod_count = instance_buffer.lod_count;
            resources.sum_sizes = (resources.instance_count/256 + 1, 0);
            resources.sum_sizes.1 = resources.sum_sizes.0/256+1;
            let instance_count = resources.instance_count;
            let lod_count = resources.lod_count;
            let sum_sizes = resources.sum_sizes;

            resources.vote_buffer.replace(render_device.create_buffer(&BufferDescriptor { 
                label: Some("Corn Vote Buffer".into()), 
                size: instance_count * 8, 
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                mapped_at_creation: false 
            })).and_then(|buffer| Some(buffer.destroy()));
            resources.sum_buffers.replace((
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 1".into()), 
                    size: sum_sizes.0 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 2".into()), 
                    size: sum_sizes.1 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false 
                }),
            )).and_then(|(a, b)| Some((a.destroy(), b.destroy())));
        }else if instance_buffer.lod_count != resources.lod_count{
            resources.lod_count = instance_buffer.lod_count;
            let lod_count = resources.lod_count;
            let sum_sizes = resources.sum_sizes;
            resources.sum_buffers.replace((
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 1".into()), 
                    size: sum_sizes.0 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 2".into()), 
                    size: sum_sizes.1 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false 
                }),
            )).and_then(|(a, b)| Some((a.destroy(), b.destroy())));
        }
        
        if READBACK_ENABLED{
            let instance_count = resources.instance_count;
            let sum_sizes = resources.sum_sizes;
            let lod_count = resources.lod_count;
            resources.readback_buffers.replace((
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Instances Readback Buffer".into()), 
                    size: instance_count*CORN_DATA_SIZE, 
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Vote Readback Buffer".into()), 
                    size: instance_count * 8, 
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Readback Buffer 1".into()), 
                    size: sum_sizes.0 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Readback Buffer 2".into()), 
                    size: sum_sizes.1 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Indirect Readback Buffer".into()), 
                    size: 20*lod_count as u64, 
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sorted Readback Buffer".into()), 
                    size: instance_count*CORN_DATA_SIZE, 
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, 
                    mapped_at_creation: false 
                })
            )).and_then(|buffers| Some((
                buffers.0.destroy(),
                buffers.1.destroy(),
                buffers.2.destroy(),
                buffers.3.destroy(),
                buffers.4.destroy(),
                buffers.5.destroy()
            )));
        }
        
        if TIMING_ENABLED{
            if resources.timing_query_set.is_none(){
                resources.timing_query_set = Some(render_device.wgpu_device().create_query_set(&QuerySetDescriptor { 
                    label: Some("Scan Prepass Timings".into()), 
                    ty: wgpu::QueryType::Timestamp, 
                    count: 2 
                }));
            }            
            if resources.timing_buffer.is_none(){
                resources.timing_buffer = Some((
                    render_device.create_buffer(&BufferDescriptor{
                        label: Some("Scan Prepass Timing Query Buffer".into()),
                        usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                        size: 16,
                        mapped_at_creation: false
                    }),
                    render_device.create_buffer(&BufferDescriptor{
                        label: Some("Scan Prepass Timing Query Buffer".into()),
                        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                        size: 16,
                        mapped_at_creation: false
                    })
                ));
            }
        }
    }
    /// Runs during prepare phase, creates the bind group necessary for this frame
    pub fn prepare_bind_group(
        mut resources: ResMut<ScanPrepassResources>, 
        instance_buffer: Res<CornInstanceBuffer>,
        render_device: Res<RenderDevice>,
        pipeline: Res<CornBufferPrePassPipeline>,
    ){
        if instance_buffer.id == resources.buffer_id {return;}
        resources.buffer_id = instance_buffer.id;
        let bind_group = [
            BindGroupEntry{
                binding: 0,
                resource: BindingResource::Buffer(instance_buffer.data_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 1,
                resource: BindingResource::Buffer(resources.vote_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 2,
                resource: BindingResource::Buffer(resources.sum_buffers.as_ref().unwrap().0.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 3,
                resource: BindingResource::Buffer(resources.sum_buffers.as_ref().unwrap().1.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 4,
                resource: BindingResource::Buffer(instance_buffer.indirect_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 5,
                resource: BindingResource::Buffer(instance_buffer.sorted_buffer.as_ref().unwrap().as_entire_buffer_binding())
            }
        ];
        resources.bind_group = Some(render_device.create_bind_group( 
            Some("Corn Scan Prepass Bind Group"), 
            &pipeline.bind_group_layout, 
            &bind_group
        ));
    }
    /// Runs during cleanup if readback is enabled, printing buffer data to the console
    pub fn finish_readback(
        mut resources: ResMut<ScanPrepassResources>, 
        render_device: Res<RenderDevice>
    ){
        if let Some((a, b, c, d, e, f)) = resources.readback_buffers.take(){
            readback_buffer::<PerCornData>("Instance Buffer:".to_string(), a, render_device.as_ref());
            readback_buffer::<[u32; 2]>("Vote Buffer:".to_string(), b, render_device.as_ref());
            readback_buffer::<u32>("Sum 1 Buffer:".to_string(), c, render_device.as_ref());
            readback_buffer::<u32>("Sum 2 Buffer:".to_string(), d, render_device.as_ref());
            readback_buffer::<[u32; 5]>("Indirect Buffer:".to_string(), e, render_device.as_ref());
            readback_buffer::<PerCornData>("Sorted Buffer:".to_string(), f, render_device.as_ref());
        }
    }
    /// Runs during cleanup if timing is enabled, printing timing data to the console
    pub fn finish_timing(
        resources: ResMut<ScanPrepassResources>, 
        render_device: Res<RenderDevice>,
        queue: Res<RenderQueue>
    ){
        if let Some(buffer) = resources.timing_buffer.as_ref(){
            let slice = buffer.1.slice(..);
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
                let raw = buffer.1
                    .slice(..).get_mapped_range()
                    .iter().map(|v| *v).collect::<Vec<u8>>();
                let data = bytemuck::cast_slice::<u8, u64>(raw.as_slice()).to_vec();
                let nanos = queue.get_timestamp_period()*(data[1] - data[0]) as f32;
                println!("Scan Prepass Took: {} nanos", nanos);
            }
            buffer.1.unmap();
        }
    }
}

pub fn readback_buffer<T: std::fmt::Debug + Pod>(message: String, buffer: Buffer, render_device: &RenderDevice){
    let slice = buffer.slice(..);
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
        let raw = buffer
            .slice(..).get_mapped_range()
            .iter().map(|v| *v).collect::<Vec<u8>>();
        let data = bytemuck::cast_slice::<u8, T>(raw.as_slice()).to_vec();
        println!("{}", message);
        for corn in data{
            println!("{:?}", corn);
        }
        println!("");
    }
    buffer.destroy();
}

/// ### Added to the rendergraph as an asynchronous step
/// - run function is called by the render phase at some point
/// - runs all vote-scan-compact compute passes
pub struct CornBufferPrepassNode{}
impl Node for CornBufferPrepassNode{
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        //get vote scan compact resources
        let resources = world.resource::<ScanPrepassResources>();
        if !resources.enabled{return Ok(());}
        let lod_cutoffs = world.resource::<LodCutoffs>();
        let lod_count: u32 = resources.lod_count;
        //get our pipeline
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<CornBufferPrePassPipeline>();
        // Setup compute pass
        let mut compute_pass = render_context.command_encoder().begin_compute_pass(
            &ComputePassDescriptor { label: Some("Vote Scan Compact Pass") }
        );
        compute_pass.set_bind_group(0, resources.bind_group.as_ref().unwrap(), &[]);
        if TIMING_ENABLED{
            compute_pass.write_timestamp(resources.timing_query_set.as_ref().unwrap(), 0);
        }
        if let Some(pipelines) = pipeline.ids.get(&lod_count){
            if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipelines[0]){
                compute_pass.set_pipeline(pipeline);
                compute_pass.set_push_constants(0, bytemuck::cast_slice(&[resources.frustum_values]));
                compute_pass.set_push_constants(80, bytemuck::cast_slice(lod_cutoffs.0.iter().map(|x| x*x).collect::<Vec<f32>>().as_slice()));
                compute_pass.dispatch_workgroups((resources.instance_count as f32 / 256.0).ceil() as u32, 1, 1);
            }
            if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipelines[1]){
                compute_pass.set_pipeline(pipeline);
                compute_pass.dispatch_workgroups((resources.sum_sizes.0 as f32 / 256.0).ceil() as u32, 1, 1);
            }
            if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipelines[2]){
                compute_pass.set_pipeline(pipeline);
                compute_pass.dispatch_workgroups((resources.sum_sizes.1 as f32 / 256.0).ceil() as u32, 1, 1);
            }
            if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipelines[3]){
                compute_pass.set_pipeline(pipeline);
                compute_pass.dispatch_workgroups((resources.instance_count as f32 / 256.0).ceil() as u32, 1, 1);
            }
        }
        if TIMING_ENABLED{
            compute_pass.write_timestamp(resources.timing_query_set.as_ref().unwrap(), 1);
        }
        drop(compute_pass);
        if TIMING_ENABLED{
            render_context.command_encoder().resolve_query_set(
                resources.timing_query_set.as_ref().unwrap(), 
                0..2,
                &resources.timing_buffer.as_ref().unwrap().0, 
                0
            );
            render_context.command_encoder().copy_buffer_to_buffer(
                &resources.timing_buffer.as_ref().unwrap().0, 0, 
                &resources.timing_buffer.as_ref().unwrap().1, 0, 16);
        }
        if READBACK_ENABLED{
            let instance_buffer = world.resource::<CornInstanceBuffer>();
            if let Some((a, b, c, d, e, f)) = resources.readback_buffers.as_ref(){
                render_context.command_encoder().copy_buffer_to_buffer(
                    instance_buffer.get_instance_buffer().unwrap(), 0, 
                    &a, 0, a.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    resources.vote_buffer.as_ref().unwrap(), 0, 
                    &b, 0, b.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    &resources.sum_buffers.as_ref().unwrap().0, 0, 
                    &c, 0, c.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    &resources.sum_buffers.as_ref().unwrap().1, 0, 
                    &d, 0, d.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    instance_buffer.get_indirect_buffer().unwrap(), 0, 
                    &e, 0, e.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    instance_buffer.get_sorted_buffer().unwrap(), 0, 
                    &f, 0, f.size()
                );   
            }
        }
        return Ok(());
    }
}

/// Pipeline for the Vote-Scan-Compact compute pass 
#[derive(Resource)]
pub struct CornBufferPrePassPipeline{
    pub ids: HashMap<u32, [CachedComputePipelineId; 4]>,
    pub bind_group_layout: BindGroupLayout,
    pub shader: Handle<Shader>
}
impl CornBufferPrePassPipeline{
    pub fn get_pipeline_descriptors(&self, lod_count: u32) -> [ComputePipelineDescriptor; 4]{
        [
        ComputePipelineDescriptor{
            label: Some("Corn Vote_Scan Pipeline".into()),
            layout: vec![self.bind_group_layout.clone()],
            push_constant_ranges: vec![
                PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..(4*(lod_count+21))}
            ],
            shader: self.shader.clone(),
            shader_defs: vec![
                ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), lod_count+1),
                ShaderDefVal::UInt("OVERRIDE_INDIRECT_COUNT".to_string(), lod_count*5),
            ],
            entry_point: "vote_scan".into()
        },
        ComputePipelineDescriptor{
            label: Some("Corn Sum 1 Pipeline".into()),
            layout: vec![self.bind_group_layout.clone()],
            push_constant_ranges: vec![
                PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..(4*(lod_count+21))}
            ],
            shader: self.shader.clone(),
            shader_defs: vec![
                ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), lod_count+1),
                ShaderDefVal::UInt("OVERRIDE_INDIRECT_COUNT".to_string(), lod_count*5),
            ],
            entry_point: "group_scan_1".into()
        },
        ComputePipelineDescriptor{
            label: Some("Corn Sum 2 Pipeline".into()),
            layout: vec![self.bind_group_layout.clone()],
            push_constant_ranges: vec![
                PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..(4*(lod_count+21))}
            ],
            shader: self.shader.clone(),
            shader_defs: vec![
                ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), lod_count+1),
                ShaderDefVal::UInt("OVERRIDE_INDIRECT_COUNT".to_string(), lod_count*5),
            ],
            entry_point: "group_scan_2".into()
        },
        ComputePipelineDescriptor{
            label: Some("Corn Compact Pipeline".into()),
            layout: vec![self.bind_group_layout.clone()],
            push_constant_ranges: vec![
                PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..(4*(lod_count+21))}
            ],
            shader: self.shader.clone(),
            shader_defs: vec![
                ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), lod_count+1),
                ShaderDefVal::UInt("OVERRIDE_INDIRECT_COUNT".to_string(), lod_count*5),
            ],
            entry_point: "compact".into()
        },
        ]
    }
}
impl FromWorld for CornBufferPrePassPipeline {
    fn from_world(world: &mut World) -> Self {
        let layout = world.resource::<RenderDevice>().create_bind_group_layout(
        &BindGroupLayoutDescriptor {
        label: Some("Corn Scan Prepass Bind Group Layout".into()),
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
                    ty: BufferBindingType::Storage { read_only: false }, 
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
                    ty: BufferBindingType::Storage { read_only: false }, 
                    has_dynamic_offset: false, 
                    min_binding_size: None },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer { 
                    ty: BufferBindingType::Storage { read_only: false }, 
                    has_dynamic_offset: false, 
                    min_binding_size: None },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer { 
                    ty: BufferBindingType::Storage { read_only: false }, 
                    has_dynamic_offset: false, 
                    min_binding_size: None },
                count: None,
            }
        ]});
        let shader = world
            .resource::<AssetServer>()
            .load("shaders/corn/vote_scan_compact.wgsl");
        return Self{ids: HashMap::default(), bind_group_layout: layout, shader};       
    }
}

/// ### Adds the vote scan compact prepass functionality to the game
pub struct MasterCornPrepassPlugin;
impl Plugin for MasterCornPrepassPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<LodCutoffs>()
            .init_resource::<LodCutoffs>()
            .add_systems(PostUpdate, LodCutoffs::update_lod_cutoffs);
        app.get_sub_app_mut(RenderApp).unwrap()
            .init_resource::<LodCutoffs>()
            .init_resource::<ScanPrepassResources>()
            .add_systems(Render, (
                ScanPrepassResources::prepare_buffers.in_set(RenderSet::PrepareResources),
                ScanPrepassResources::prepare_bind_group.in_set(RenderSet::PrepareBindGroups)
            )).add_systems(ExtractSchedule, LodCutoffs::extract_lod_cutoffs);
        if READBACK_ENABLED{
            app.sub_app_mut(RenderApp).add_systems(Render, ScanPrepassResources::finish_readback.in_set(RenderSet::Cleanup));
        }
        if TIMING_ENABLED{
            app.sub_app_mut(RenderApp).add_systems(Render, ScanPrepassResources::finish_timing.in_set(RenderSet::Cleanup));
        }
        let mut binding = app.get_sub_app_mut(RenderApp).unwrap()
            .world.get_resource_mut::<RenderGraph>().unwrap();
        let graph = binding
            .get_sub_graph_mut(core_3d::graph::NAME).unwrap();
        graph.add_node("vote_scan_compact", CornBufferPrepassNode{});
        graph.add_node_edge("vote_scan_compact", SHADOW_PASS);
    }
    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CornBufferPrePassPipeline>();
    }
}
