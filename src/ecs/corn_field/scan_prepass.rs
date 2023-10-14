use bevy::{
    prelude::*, 
    render::{
        render_resource::*, 
        renderer::{RenderDevice, RenderContext}, 
        RenderApp, 
        RenderSet, 
        Render, 
        render_graph::{Node, RenderGraphContext, RenderGraph}, view::ExtractedView
    }, pbr::draw_3d_graph::node::SHADOW_PASS, core_pipeline::core_3d
};
use bytemuck::{Pod, Zeroable};
use crate::ecs::main_camera::MainCamera;
use super::CornInstanceBuffer;

/// Respresents frustum structure in compute shader sans lod distance cutoffs
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct FrustumValues {
    pub left_normal: Vec2,
    pub right_normal: Vec2,
    pub offset: Vec2,
    _padding: Vec2
}

/// ### Keeps hold of all of the vote-scan-compact shader resources
#[derive(Resource, Default)]
pub struct VoteScanCompactBuffers{
    vote_scan: Option<Buffer>,
    count_1: Option<Buffer>,
    count_1_size: u32,
    count_2: Option<Buffer>,
    count_2_size: u32,
    instance_count: u32,
    lod_count: u32,
    bind_group: Option<BindGroup>,
    frustum_values: FrustumValues,
    lod_cutoffs: Vec<f32>,
    enabled: bool
}
impl VoteScanCompactBuffers{
    pub fn init(
        &mut self, 
        render_device: &RenderDevice,
        instance_buffer: &CornInstanceBuffer,
        pipeline: &CornBufferPrePassPipeline
    ){
        self.lod_count = instance_buffer.lod_count;
        self.lod_cutoffs = (0..self.lod_count).map(|i| 
            ((50.0 / (self.lod_count as f32))*((i+1) as f32)).powi(2)
        ).collect();
        self.instance_count = instance_buffer.data_count;
        self.count_1_size = self.instance_count/256+1;
        self.count_2_size = self.count_1_size/256+1;
        if let Some(buffer) = self.vote_scan.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.count_1.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.count_2.as_ref() {buffer.destroy();}
        self.vote_scan = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Vote Scan Buffer"), 
            size: self.instance_count as u64 * 8u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.count_1 = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Intermediate Count Buffer 1"), 
            size: self.count_1_size as u64 * 4u64*(self.lod_count+1) as u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.count_2 = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Intermediate Count Buffer 2"), 
            size: self.count_2_size as u64 * 4u64*(self.lod_count+1) as u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        let bind_group = [
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
                resource: BindingResource::Buffer(self.count_1.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 3,
                resource: BindingResource::Buffer(self.count_2.as_ref().unwrap().as_entire_buffer_binding())
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
            label: Some("Corn Vote Buffer Bind Group"), 
            layout: &pipeline.bind_group_layout, 
            entries: &bind_group
        }));
        self.enabled = true;
    }
    pub fn update_size(
        &mut self, 
        render_device: &RenderDevice,
        instance_buffer: &CornInstanceBuffer,
        pipeline: &CornBufferPrePassPipeline
    ){
        self.lod_count = instance_buffer.lod_count;
        self.lod_cutoffs = (0..self.lod_count).map(|i| 
            ((50.0 / (self.lod_count as f32))*((i+1) as f32)).powi(2)
        ).collect();
        self.instance_count = instance_buffer.data_count;
        self.count_1_size = self.instance_count/256+1;
        self.count_2_size = self.count_1_size/256+1;
        if let Some(buffer) = self.vote_scan.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.count_1.as_ref() {buffer.destroy();}
        if let Some(buffer) = self.count_2.as_ref() {buffer.destroy();}
        self.vote_scan = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Vote Scan Buffer"), 
            size: self.instance_count as u64 * 8u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.count_1 = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Intermediate Count Buffer 1"), 
            size: self.count_1_size as u64 * 4u64*(self.lod_count+1) as u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.count_2 = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Intermediate Count Buffer 2"), 
            size: self.count_2_size as u64 * 4u64*(self.lod_count+1) as u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        let bind_group = [
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
                resource: BindingResource::Buffer(self.count_1.as_ref().unwrap().as_entire_buffer_binding())
            },
            BindGroupEntry{
                binding: 3,
                resource: BindingResource::Buffer(self.count_2.as_ref().unwrap().as_entire_buffer_binding())
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
            label: Some("Corn Vote Buffer Bind Group"), 
            layout: &pipeline.bind_group_layout, 
            entries: &bind_group
        }));
    }
    pub fn destroy(&mut self){
        if !self.enabled {return;}
        if let Some(buffer) = self.vote_scan.as_ref(){buffer.destroy(); self.vote_scan = None;}
        if let Some(buffer) = self.count_1.as_ref(){buffer.destroy(); self.count_1 = None;}
        if let Some(buffer) = self.count_2.as_ref(){buffer.destroy(); self.count_2 = None;}
        self.count_1_size = 0; self.count_2_size = 0;
        self.instance_count = 0; self.lod_count = 0;
        self.bind_group = None;
        self.enabled = false;
    }
    pub fn cleanup(
        &mut self, 
        instance_buffer: &CornInstanceBuffer,
        render_device: &RenderDevice,
        prepass_pipeline: &CornBufferPrePassPipeline
    ){
        if !instance_buffer.enabled && self.enabled {self.destroy(); return;}
        if instance_buffer.enabled && !self.enabled {self.init(
            render_device, instance_buffer, prepass_pipeline
        ); return;}
        if self.instance_count != instance_buffer.data_count || self.lod_count != instance_buffer.lod_count {
            self.update_size(render_device, instance_buffer, prepass_pipeline);
        }
    }
}
/// ### Runs during cleanup
/// Assures that the vote-scan-compact buffer mirrors the instace buffer's status
pub fn prepare_vote_scan_compact_pass(
    mut buffers: ResMut<VoteScanCompactBuffers>,
    mut pipeline: ResMut<CornBufferPrePassPipeline>,
    mut pipelines: ResMut<SpecializedComputePipelines<CornBufferPrePassPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    camera: Query<&ExtractedView, With<MainCamera>>
){
    if !buffers.enabled{
        return;
    }
    if pipeline.lod_count != buffers.lod_count{
        let ids = vec![
            pipelines.specialize(&pipeline_cache, &pipeline, ("vote_scan".to_string(), buffers.lod_count)),
            pipelines.specialize(&pipeline_cache, &pipeline, ("group_scan_1".to_string(), buffers.lod_count)),
            pipelines.specialize(&pipeline_cache, &pipeline, ("group_scan_2".to_string(), buffers.lod_count)),
            pipelines.specialize(&pipeline_cache, &pipeline, ("compact".to_string(), buffers.lod_count)),
        ];
        pipeline.ids = ids;
        pipeline.lod_count = buffers.lod_count;
    }
    //Calculate frustum settings
    buffers.frustum_values._padding = Vec2::new(4.0, 4.0);
    let view = camera.single();
    let trans = view.transform;
    let proj = view.projection*trans.compute_matrix();
    let lrbtnf = extract_planes_from_projmat(proj);
    let mut lv = Vec2::new(-lrbtnf[0][2], lrbtnf[0][0]);
    let mut rv = Vec2::new(-lrbtnf[1][2], lrbtnf[1][0]);
    if lv.dot(Vec2::new(trans.left().x, trans.left().z)) < 0.0 {lv *= -1.0;}
    if rv.dot(Vec2::new(trans.right().x, trans.right().z)) < 0.0 {rv *= -1.0;}
    buffers.frustum_values.left_normal = Vec2::new(-lv.y, lv.x).normalize();
    buffers.frustum_values.right_normal = Vec2::new(rv.y, -rv.x).normalize();
    buffers.frustum_values.offset = Vec2::new(trans.translation().x, trans.translation().z);
}
pub fn extract_planes_from_projmat(mat: Mat4) -> [[f32; 4]; 6]{
    let mut lrbtnf = [[0.0; 4]; 6];
    for i in (0..4).rev() { 
        lrbtnf[0][i] = mat.col(i)[3] + mat.col(i)[0]; 
        lrbtnf[1][i] = mat.col(i)[3] - mat.col(i)[0];
        lrbtnf[2][i] = mat.col(i)[3] + mat.col(i)[1]; 
        lrbtnf[3][i] = mat.col(i)[3] - mat.col(i)[1]; 
        lrbtnf[4][i] = mat.col(i)[3] + mat.col(i)[2]; 
        lrbtnf[5][i] = mat.col(i)[3] - mat.col(i)[2]; 
    }
    return lrbtnf;
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
        let vote_res = world.get_resource::<VoteScanCompactBuffers>();
        if vote_res.is_none() {return Ok(());}
        let vote_res = vote_res.unwrap();
        if !vote_res.enabled {return Ok(());}
        //get our pipeline
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<CornBufferPrePassPipeline>();
        if pipeline.ids.len() == 0 {return Ok(());}
        // Setup compute pass
        let mut compute_pass = render_context.command_encoder().begin_compute_pass(
            &ComputePassDescriptor { label: Some("Vote Scan Compact Pass") }
        );
        compute_pass.set_bind_group(0, vote_res.bind_group.as_ref().unwrap(), &[]);
        // Run each of the four compute shaders
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[0]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_push_constants(0, bytemuck::cast_slice(&[vote_res.frustum_values]));
            compute_pass.set_push_constants(32, bytemuck::cast_slice(vote_res.lod_cutoffs.as_slice()));
            compute_pass.dispatch_workgroups((vote_res.instance_count as f32 / 256.0).ceil() as u32, 1, 1);
        }
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[1]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.dispatch_workgroups((vote_res.count_1_size as f32 / 256.0).ceil() as u32, 1, 1);
        }
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[2]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.dispatch_workgroups((vote_res.count_2_size as f32 / 256.0).ceil() as u32, 1, 1);
        }
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.ids[3]){
            compute_pass.set_pipeline(pipeline);
            compute_pass.dispatch_workgroups((vote_res.instance_count as f32 / 256.0).ceil() as u32, 1, 1);
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
            push_constant_ranges: vec![
                PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..(4*(key.1+9))}
            ],
            shader: self.shader.clone(),
            shader_defs: vec![
                ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), key.1 as u32 + 1),
                ShaderDefVal::UInt("OVERRIDE_INDIRECT_COUNT".to_string(), key.1 as u32*5),
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
