use bevy::{
    prelude::*, 
    render::{
        render_resource::*, 
        renderer::{RenderDevice, RenderContext}, 
        RenderApp, 
        RenderSet, 
        Render, 
        render_graph::{Node, RenderGraphContext, RenderGraph}, view::ExtractedView, Extract
    }, pbr::draw_3d_graph::node::SHADOW_PASS, core_pipeline::core_3d, utils::hashbrown::HashMap
};
use bytemuck::{Pod, Zeroable};
use crate::{ecs::main_camera::MainCamera, prelude::corn_model::CornMeshes};
use super::CornInstanceBuffer;

/// Respresents frustum structure in compute shader sans lod distance cutoffs
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
#[repr(C)]
pub struct FrustumValues {
    pub col1: Vec4,
    pub col2: Vec4,
    pub col3: Vec4,
    pub col4: Vec4,
    pub offset: Vec4
}
impl From<&ExtractedView> for FrustumValues{
    fn from(value: &ExtractedView) -> Self {
        let proj = value.projection*value.transform.compute_matrix().inverse();
        Self{
            col1: proj.col(0),
            col2: proj.col(1),
            col3: proj.col(2),
            col4: proj.col(3),
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
    enabled: bool
}
impl ScanPrepassResources{
    /// Runs during prepare phase, creates all resources necessary for this frame
    pub fn prepare_resources(
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

        if instance_buffer.id == resources.buffer_id {return;}
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
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false 
            })).and_then(|buffer| Some(buffer.destroy()));
            resources.sum_buffers.replace((
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 1".into()), 
                    size: sum_sizes.0 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 2".into()), 
                    size: sum_sizes.1 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE, 
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
                    usage: BufferUsages::STORAGE, 
                    mapped_at_creation: false 
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Sum Buffer 2".into()), 
                    size: sum_sizes.1 * 4 *(lod_count+1) as u64, 
                    usage: BufferUsages::STORAGE, 
                    mapped_at_creation: false 
                }),
            )).and_then(|(a, b)| Some((a.destroy(), b.destroy())));
        }
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
        resources.bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor { 
            label: Some("Corn Scan Prepass Bind Group"), 
            layout: &pipeline.bind_group_layout, 
            entries: &bind_group
        }));
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
pub struct MasterCornPrepassPlugin{}
impl Plugin for MasterCornPrepassPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<LodCutoffs>()
            .init_resource::<LodCutoffs>()
            .add_systems(PostUpdate, LodCutoffs::update_lod_cutoffs);
        app.get_sub_app_mut(RenderApp).unwrap()
            .init_resource::<LodCutoffs>()
            .init_resource::<ScanPrepassResources>()
            .add_systems(Render, 
                ScanPrepassResources::prepare_resources.in_set(RenderSet::Prepare)
            ).add_systems(ExtractSchedule, LodCutoffs::extract_lod_cutoffs);
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
