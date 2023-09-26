use bevy::{
    prelude::*, 
    render::{
        render_resource::*, 
        renderer::{RenderDevice, RenderContext}, 
        RenderApp, 
        RenderSet, 
        Render, 
        render_graph::{Node, RenderGraphContext, RenderGraph}
    }, pbr::draw_3d_graph::node::SHADOW_PASS, core_pipeline::core_3d
};
use super::CornInstanceBuffer;
/// ### Keeps hold of all of the vote-scan-compact shader resources
#[derive(Resource, Default)]
pub struct VoteScanCompactBuffers{
    vote_scan: Option<Buffer>,
    count_buffers: Option<(Buffer, Buffer)>,
    count_sizes: (u32, u32),
    instance_count: u32,
    lod_count: u32,
    enabled: bool,
    bind_group: Option<BindGroup>
}
impl VoteScanCompactBuffers{
    pub fn init(
        &mut self, 
        render_device: &RenderDevice,
        instance_buffer: &CornInstanceBuffer,
        pipeline: &CornBufferPrePassPipeline
    ){
        self.lod_count = instance_buffer.lod_count;
        self.instance_count = instance_buffer.data_count;
        self.enabled = true;
        self.count_sizes = (self.instance_count/256+1, (self.instance_count/256+1)/256+1);
        self.vote_scan = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Vote Scan Buffer"), 
            size: self.instance_count as u64 * 8u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.count_buffers = Some((
            render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Intermediate Count Buffer 1"), 
                size: self.count_sizes.0 as u64 * 4u64*(self.lod_count+1) as u64, 
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false
            }),
            render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Intermediate Count Buffer 2"), 
                size: self.count_sizes.1 as u64 * 4u64*(self.lod_count+1) as u64, 
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false
            })
        ));
        let init_bind_group = [
            BindGroupEntry{
                binding: 0,
                resource: BindingResource::Buffer(instance_buffer.data_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 1,
                resource: BindingResource::Buffer(self.vote_scan.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 2,
                resource: BindingResource::Buffer(self.count_buffers.as_ref().unwrap().0.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 3,
                resource: BindingResource::Buffer(self.count_buffers.as_ref().unwrap().1.as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 4,
                resource: BindingResource::Buffer(instance_buffer.indirect_buffer.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 5,
                resource: BindingResource::Buffer(instance_buffer.index_buffer.as_ref().unwrap().as_entire_buffer_binding())
            }
        ];
        self.bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Init Buffer Bind Group"), 
            layout: &pipeline.bind_group_layout, 
            entries: &init_bind_group
        }));
        if !instance_buffer.ready_to_render(){return;}
    }
    pub fn update_size(
        &mut self, 
        render_device: &RenderDevice,
        instance_buffer: &CornInstanceBuffer,
        pipeline: &CornBufferPrePassPipeline
    ){
        self.destroy();
        self.init(render_device, instance_buffer, pipeline);
    }
    pub fn destroy(&mut self){
        if !self.enabled {return;}
        if let Some(buffer) = self.vote_scan.as_ref(){buffer.destroy(); self.vote_scan = None;}
        if let Some((buffer, buffer2)) = self.count_buffers.as_ref(){
            buffer.destroy(); buffer2.destroy(); self.count_buffers = None;
        }
        self.count_sizes = (0, 0);
        self.instance_count = 0;
        self.lod_count = 0;
        self.enabled = false;
    }
}
/// ### Runs during cleanup
/// Assures that the vote-scan-compact buffer mirrors the instace buffer's status
pub fn prepare_vote_scan_compact_pass(
    mut buffers: ResMut<VoteScanCompactBuffers>,
    instance_buffer: Res<CornInstanceBuffer>,
    render_device: Res<RenderDevice>,
    mut pipeline: ResMut<CornBufferPrePassPipeline>,
    mut pipelines: ResMut<SpecializedComputePipelines<CornBufferPrePassPipeline>>,
    pipeline_cache: Res<PipelineCache>
){
    if !instance_buffer.ready_to_render(){
        buffers.destroy();
        return;
    }
    if !buffers.enabled{
        buffers.init(&render_device, &instance_buffer, &pipeline);
        let ids = vec![
            pipelines.specialize(&pipeline_cache, &pipeline, ("vote_scan".to_string(), instance_buffer.lod_count)),
            pipelines.specialize(&pipeline_cache, &pipeline, ("group_scan_1".to_string(), instance_buffer.lod_count)),
            pipelines.specialize(&pipeline_cache, &pipeline, ("group_scan_2".to_string(), instance_buffer.lod_count)),
            pipelines.specialize(&pipeline_cache, &pipeline, ("compact".to_string(), instance_buffer.lod_count)),
        ];
        pipeline.ids = ids;
        return;
    }
    if instance_buffer.data_count != buffers.instance_count || instance_buffer.lod_count != buffers.lod_count{
        let update_pipeline_ids = instance_buffer.lod_count != buffers.lod_count;
        buffers.update_size(&render_device, &instance_buffer, &pipeline);
        if update_pipeline_ids{
            let ids = vec![
                pipelines.specialize(&pipeline_cache, &pipeline, ("vote_scan".to_string(), instance_buffer.lod_count)),
                pipelines.specialize(&pipeline_cache, &pipeline, ("group_scan_1".to_string(), instance_buffer.lod_count)),
                pipelines.specialize(&pipeline_cache, &pipeline, ("group_scan_2".to_string(), instance_buffer.lod_count)),
                pipelines.specialize(&pipeline_cache, &pipeline, ("compact".to_string(), instance_buffer.lod_count)),
            ];
            pipeline.ids = ids;
        }
    }
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
        let buffers = world.get_resource::<VoteScanCompactBuffers>();
        if buffers.is_none() {return Ok(());}
        let buffers = buffers.unwrap();
        if !buffers.enabled || buffers.bind_group.is_none(){return Ok(());}
        //get our pipeline
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<CornBufferPrePassPipeline>();
        if pipeline.ids.len() == 0 {return Ok(());}
        let mut compute_pass = render_context.command_encoder().begin_compute_pass(
            &ComputePassDescriptor { label: Some("Vote Scan Compact Pass") }
        );
        compute_pass.set_bind_group(0, buffers.bind_group.as_ref().unwrap(), &[]);
        if let Some(pipeline2) = pipeline_cache.get_compute_pipeline(pipeline.ids[0]){
            compute_pass.set_pipeline(pipeline2);
            compute_pass.dispatch_workgroups((buffers.instance_count as f32 / 256.0).ceil() as u32, 1, 1);
        }
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[1]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.dispatch_workgroups((buffers.count_sizes.0 as f32 / 256.0).ceil() as u32, 1, 1);
        }
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[2]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.dispatch_workgroups((buffers.count_sizes.1 as f32 / 256.0).ceil() as u32, 1, 1);
        }
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[3]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.dispatch_workgroups((buffers.instance_count as f32 / 256.0).ceil() as u32, 1, 1);
        }
        return Ok(());
    }
}
/// Pipeline for the Vote-Scan-Compact compute pass 
#[derive(Resource)]
pub struct CornBufferPrePassPipeline{
    pub ids: Vec<CachedComputePipelineId>,
    pub bind_group_layout: BindGroupLayout,
    pub lod_count: u32,
    pub shader: Handle<Shader>
}
impl FromWorld for CornBufferPrePassPipeline {
    fn from_world(world: &mut World) -> Self {
        let layout = world.resource::<RenderDevice>().create_bind_group_layout(
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
        return Self{ids: vec![], bind_group_layout: layout, lod_count: 0, shader};       
    }
}
impl SpecializedComputePipeline for CornBufferPrePassPipeline{
    type Key = (String, u32);

    fn specialize(&self, key: Self::Key) -> ComputePipelineDescriptor {
        ComputePipelineDescriptor{
            label: None,
            layout: vec![self.bind_group_layout.clone()],
            push_constant_ranges: vec![],
            shader: self.shader.clone(),
            shader_defs: vec![
                ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), key.1 as u32 + 1),
                ShaderDefVal::UInt("OVERRIDE_INDIRECT_COUNT".to_string(), key.1 as u32*5)
            ],
            entry_point: key.0.into()
        }
    }
}
/// ### Adds the vote scan compact prepass functionality to the game
pub struct CornBufferPrepassPlugin{}
impl Plugin for CornBufferPrepassPlugin{
    fn build(&self, app: &mut App) {
        app.get_sub_app_mut(RenderApp).unwrap()
            .init_resource::<VoteScanCompactBuffers>()
            .init_resource::<SpecializedComputePipelines<CornBufferPrePassPipeline>>()
            .add_systems(Render, 
                prepare_vote_scan_compact_pass.in_set(RenderSet::Prepare)
            );
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
