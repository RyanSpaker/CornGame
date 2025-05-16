//! Traditional Scan Prepass Algorithm. Data is stored in an instance buffer, voted on, and then copied to a vertex buffer
use std::num::NonZero;
use bevy::{
    core_pipeline::core_3d::graph::Core3d, pbr::graph::NodePbr, prelude::*, 
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin}, 
        render_graph::*, render_resource::*, 
        renderer::{RenderContext, RenderDevice}, 
        view::ExtractedView, Render, RenderApp, RenderSet
    }
};
use bytemuck::{Pod, Zeroable};
use wgpu::{BindGroupLayoutEntry, BindingType, BufferBindingType, ShaderStages};
use wgpu_types::BufferDescriptor;
use crate::ecs::{cameras::MainCamera, corn::CornField};
use super::super::{CornLoaded, GlobalLodCutoffs, IndirectBuffer, InstanceBuffer, VertexInstanceBuffer, LOD_COUNT};

/// Struct mirroring the config data needed for the vote-scan-compact shaders. Passed in as a buffer
#[derive(Clone, Copy, Default, Debug, Zeroable, Pod, ShaderType)]
#[repr(C)]
pub struct ConfigData{
    field_to_world: Mat4,
    field_to_clip: Mat4,
    cam_pos_field: Vec4
}
impl ConfigData{
    const DATA_SIZE: NonZero<u64> = NonZero::new(144).unwrap();
}

/// Pipeline resources for the 4 vote-scan-compact shaders
#[derive(Debug, Clone, Resource)]
pub struct VoteScanPipelineResources{
    pub layout: BindGroupLayout,
    pub pipelines: Vec<CachedComputePipelineId>,
    pub shader: Handle<Shader>
}
impl FromWorld for VoteScanPipelineResources{
    fn from_world(world: &mut World) -> Self {
        let shader: Handle<Shader> = world.resource::<AssetServer>().load("shaders/corn/scan_prepass.wgsl");
        let layout = world.resource::<RenderDevice>().create_bind_group_layout(
            Some("Scan Prepass BindGroup Layout"), 
            [false, false, false, false, false, false, true].into_iter().enumerate()
                .map(|(binding, uniform)| BindGroupLayoutEntry{
                    binding: binding as u32, 
                    visibility: ShaderStages::COMPUTE,
                    count: None,
                    ty: BindingType::Buffer { 
                        ty: if uniform {BufferBindingType::Uniform} else {BufferBindingType::Storage { read_only: binding==0 }}, 
                        has_dynamic_offset: false, 
                        min_binding_size: None 
                    }
                }).collect::<Vec<BindGroupLayoutEntry>>().as_slice()
        );
        let cache = world.resource::<PipelineCache>();
        let mut pipelines = vec![];
        for i in 0..4{
            pipelines.push(cache.queue_compute_pipeline(ComputePipelineDescriptor{
                label: Some("Scan Prepass Vote Stage".into()),
                layout: vec![layout.clone()],
                push_constant_ranges: vec![PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..(4*LOD_COUNT)}],
                shader: shader.clone(),
                shader_defs: vec![
                    ShaderDefVal::UInt("OVERRIDE_LOD_COUNT".to_string(), LOD_COUNT)
                ],
                entry_point: match i{
                    0 => "vote_scan",
                    1 => "group_scan",
                    2 => "group_scan2",
                    _ => "compact"
                }.into(),
                zero_initialize_workgroup_memory: true
            }));
        }
        Self{layout, pipelines, shader}
    }
}

/// Per field LOD Cutoff info
#[derive(Default, Clone, Debug, Component, Reflect, ExtractComponent)]
#[reflect(Component)]
pub enum PerFieldLodCutoffs{
    #[default] Global,
    Custom([f32; LOD_COUNT as usize])
}
impl PerFieldLodCutoffs{
    fn insert_default(query: Query<Entity, (With<CornField>, Without<Self>)>, mut commands: Commands){
        for entity in query.iter(){
            commands.entity(entity).insert(Self::Global);
        }
    }
}

/// Component which holds the vote scan buffers
#[derive(Component)]
pub struct VoteScanBuffers{
    pub vote: Buffer,
    pub groups: (Buffer, Buffer),
    pub config: Buffer,
    pub data_upload: Buffer
}
impl VoteScanBuffers{
    fn spawn_scan_buffers(
        query: Query<(Entity, &InstanceBuffer), (With<CornLoaded>, Without<Self>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, InstanceBuffer(_, count)) in query.iter(){
            let vote_buffer = render_device.create_buffer(&BufferDescriptor{
                label: Some("Corn Field Vote Buffer"),
                size: count*8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false
            });
            let group1_size = count.div_ceil(256);
            let group2_size = group1_size.div_ceil(256);
            if group2_size > 256 {panic!("Too much corn in a single entity. total.div_ceil(256).div_ceil(256) > 256")}
            let group1_buffer = render_device.create_buffer(&BufferDescriptor{
                label: Some("Corn Field Group 1 Buffer"),
                size: group1_size*4*LOD_COUNT as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false
            });
            let group2_buffer = render_device.create_buffer(&BufferDescriptor{
                label: Some("Corn Field Group 2 Buffer"),
                size: group2_size*4*LOD_COUNT as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false
            });
            let config = render_device.create_buffer(&BufferDescriptor { 
                label: Some("Corn Field Scan Prepass Config Buffer"), 
                size: ConfigData::DATA_SIZE.into(), 
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
                mapped_at_creation: false 
            });
            let data_upload = render_device.create_buffer(&BufferDescriptor { 
                label: Some("Corn Field Scan Prepass Config Buffer Data Upload Buffer"), 
                size: ConfigData::DATA_SIZE.into(), 
                usage: BufferUsages::COPY_SRC, 
                mapped_at_creation: false 
            });
            commands.entity(entity).insert(VoteScanBuffers{
                vote: vote_buffer, groups: (group1_buffer, group2_buffer), config, data_upload
            });
        }
    }
    fn update_config(
        mut query: Query<(&mut Self, &GlobalTransform)>,
        camera: Query<&ExtractedView, With<MainCamera>>,
        render_device: Res<RenderDevice>
    ){
        let Ok(view) = camera.get_single() else {return;};
        let cam_pos = view.world_from_view.translation().extend(1.0);
        let w2c = view.clip_from_view*view.world_from_view.compute_matrix().inverse();

        for (mut buffers, transform) in query.iter_mut(){
            let field_to_world = transform.compute_matrix();
            let field_to_clip = w2c*field_to_world;
            let cam_pos_field = field_to_world.inverse().mul_vec4(cam_pos);
            buffers.data_upload = render_device.create_buffer_with_data(&BufferInitDescriptor { 
                label: Some("Vote Scan Compact Config Buffer Data Upload"), 
                contents: bytemuck::cast_slice::<ConfigData, u8>(&[ConfigData{field_to_clip, field_to_world, cam_pos_field}]), 
                usage: BufferUsages::COPY_SRC
            });
        }
    }
}

/// Component which holds the bind group as well as the dispatch count
#[derive(Debug, Clone, Component)]
pub struct VoteScanBindGroup(pub BindGroup, pub [u32; 4]);
impl VoteScanBindGroup{
    fn spawn_scan_bindgroup(
        query: Query<(Entity, &VoteScanBuffers, &InstanceBuffer, &IndirectBuffer, &VertexInstanceBuffer), Without<Self>>,
        pipeline: Res<VoteScanPipelineResources>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for(entity, scan, instance, indirect, vertex) in query.iter(){
            let bindgroup = render_device.create_bind_group(
                Some("Corn Field Scan Prepass Bind Group"), 
                &pipeline.layout, 
                &[
                    BindGroupEntry{binding: 0, resource: instance.0.as_entire_binding()},
                    BindGroupEntry{binding: 1, resource: scan.vote.as_entire_binding()},
                    BindGroupEntry{binding: 2, resource: scan.groups.0.as_entire_binding()},
                    BindGroupEntry{binding: 3, resource: scan.groups.1.as_entire_binding()},
                    BindGroupEntry{binding: 4, resource: indirect.0.as_entire_binding()},
                    BindGroupEntry{binding: 5, resource: vertex.0.as_entire_binding()},
                    BindGroupEntry{binding: 6, resource: scan.config.as_entire_binding()},
                ]
            );
            let a = instance.1.div_ceil(256); let b = a.div_ceil(256); let c = b.div_ceil(256);
            let dispatch = [a as u32, b as u32,c as u32, a as u32];
            commands.entity(entity).insert(VoteScanBindGroup(bindgroup, dispatch));
        }
    }
}


/// Render Graph Label for Init Operations
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, RenderLabel)]
struct VoteScanStage;
/// This is the render graph node which executes the Scan Prepass
#[derive(Debug, Default, Clone)]
pub struct VoteScanNode{
    ready_entities: Vec<Entity>
}
impl bevy::render::render_graph::Node for VoteScanNode{
    fn update(&mut self, world: &mut World) {
        let mut query: _ = world.query_filtered::<Entity, (With<CornLoaded>, With<VoteScanBindGroup>)>();
        self.ready_entities = query.iter(world).collect();
    }
    fn run<'w>(
        &self, _graph: &mut RenderGraphContext, render_context: &mut RenderContext<'w>, world: &'w World,
    ) -> Result<(), NodeRunError> {
        let global_cutoffs = world.resource::<GlobalLodCutoffs>();
        // Get corn field data. Bind Group, dispatch count, Lod Push Constants, Config Buffer src/dst
        let field_data: Vec<(BindGroup, [u32; 4], Vec<u8>, (Buffer, Buffer))> = self.ready_entities.iter().filter_map(|entity| {
            let Some(VoteScanBindGroup(bindgroup, dispatch)) = world.get::<VoteScanBindGroup>(*entity) else {return None;};
            let Some(buffers) = world.get::<VoteScanBuffers>(*entity) else {return None;};
            let lods = match world.get::<PerFieldLodCutoffs>(*entity) {
                None => {return None;},
                Some(PerFieldLodCutoffs::Custom(l)) => l.clone(),
                Some(PerFieldLodCutoffs::Global) => global_cutoffs.0.clone()
            };
            let bytes = bytemuck::cast_slice::<f32, u8>(&lods).to_owned();
            Some((bindgroup.clone(), dispatch.to_owned(), bytes, (buffers.data_upload.clone(), buffers.config.clone())))
        }).collect();
        if field_data.is_empty() {return Ok(());}
        // Get resources
        let resources = world.resource::<VoteScanPipelineResources>();
        let cache = world.resource::<PipelineCache>();
        // Get pipelines
        let mut pipelines = vec![];
        for pipeline in resources.pipelines.iter(){
            let Some(pipeline) = cache.get_compute_pipeline(*pipeline) else {return Ok(());};
            pipelines.push(pipeline);
        }
        // Copy Buffers
        for (_, _, _, (src, dst)) in field_data.iter(){
            render_context.command_encoder().copy_buffer_to_buffer(
                src, 0, dst, 0, src.size()
            );
        }
        // Start Compute Pass
        let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor { 
            label: Some("Scan Prepass Compute Pass"), timestamp_writes: None 
        });
        // Vote
        compute_pass.set_pipeline(pipelines[0]);
        for (bindgroup, dispatch, bytes, _) in field_data.iter(){
            compute_pass.set_bind_group(0, bindgroup, &[]);
            compute_pass.set_push_constants(0, bytes.as_slice());
            compute_pass.dispatch_workgroups(dispatch[0], 1, 1);
        }
        // Group 1
        compute_pass.set_pipeline(pipelines[1]);
        for (bindgroup, dispatch, bytes, _) in field_data.iter(){
            compute_pass.set_bind_group(0, bindgroup, &[]);
            compute_pass.set_push_constants(0, bytes.as_slice());
            compute_pass.dispatch_workgroups(dispatch[1], 1, 1);
        }
        // Group 2
        compute_pass.set_pipeline(pipelines[2]);
        for (bindgroup, dispatch, bytes, _) in field_data.iter(){
            compute_pass.set_bind_group(0, bindgroup, &[]);
            compute_pass.set_push_constants(0, bytes.as_slice());
            compute_pass.dispatch_workgroups(dispatch[2], 1, 1);
        }
        // Compact
        compute_pass.set_pipeline(pipelines[3]);
        for (bindgroup, dispatch, bytes, _) in field_data.iter(){
            compute_pass.set_bind_group(0, bindgroup, &[]);
            compute_pass.set_push_constants(0, bytes.as_slice());
            compute_pass.dispatch_workgroups(dispatch[3], 1, 1);
        }
        Ok(())
    }
}

/// Adds VoteScan Prepass Functionality
pub struct VoteScanPlugin;
impl Plugin for VoteScanPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<PerFieldLodCutoffs>()
            .add_plugins(ExtractComponentPlugin::<PerFieldLodCutoffs>::default())
        .sub_app_mut(RenderApp)
            .add_systems(Render, (
                PerFieldLodCutoffs::insert_default.in_set(RenderSet::Prepare),
                (
                    VoteScanBuffers::spawn_scan_buffers,
                    VoteScanBuffers::update_config
                ).chain().in_set(RenderSet::PrepareResources),
                VoteScanBindGroup::spawn_scan_bindgroup.in_set(RenderSet::PrepareBindGroups)
            ));
        // Add Scan Node to RenderGraph
        let mut render_graph = app.sub_app_mut(RenderApp)
            .world_mut().resource_mut::<RenderGraph>();
        let graph = render_graph.sub_graph_mut(Core3d);
        graph.add_node(VoteScanStage, VoteScanNode::default());
        graph.add_node_edge(VoteScanStage, NodePbr::ShadowPass);

        #[cfg(debug_assertions)]
        app.add_plugins(readback::ReadbackPlugin);
    }
    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<VoteScanPipelineResources>();
    }
}

#[cfg(debug_assertions)]
pub mod readback{
    use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
    use bevy::{core_pipeline::core_3d::graph::Core3d, prelude::*, render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin}, 
        render_graph::{RenderGraph, RenderLabel}, 
        render_resource::Buffer, renderer::RenderDevice, 
        Render, RenderApp, RenderSet
    }};
    use bytemuck::Pod;
    use wgpu::{BufferUsages, Maintain, MapMode};
    use wgpu_types::BufferDescriptor;
    use crate::ecs::corn::{CornData, CornLoaded, IndirectBuffer, InstanceBuffer, VertexInstanceBuffer, LOD_COUNT};
    use super::{ConfigData, VoteScanBuffers, VoteScanStage};
    
    pub fn readback_buffer<T: std::fmt::Debug + Pod>(message: String, buffer: &Buffer, render_device: &RenderDevice){
        let slice = buffer.slice(..);
        let flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let flag_captured = flag.clone();
        slice.map_async(MapMode::Read, move |v|{
            if v.is_ok() {flag_captured.store(true, Ordering::Relaxed);}
        });
        render_device.poll(Maintain::Wait);
        if flag.load(Ordering::Relaxed) {
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
        buffer.unmap(); 
    }

    #[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Component, ExtractComponent)]
    #[reflect(Component)]
    pub struct ReadbackVoteScan;

    #[derive(Debug, Clone, Component)]
    pub struct ReadbackVoteScanBuffers{
        instance: Buffer,
        vote: Buffer,
        groups: (Buffer, Buffer),
        indirect: Buffer,
        vertex: Buffer,
        config: Buffer
    }
    impl ReadbackVoteScanBuffers{
        fn create_buffers(
            query: Query<(
                Entity, &InstanceBuffer, &VertexInstanceBuffer, &IndirectBuffer, &VoteScanBuffers
            ), (With<CornLoaded>, With<ReadbackVoteScan>, Without<Self>)>,
            render_device: Res<RenderDevice>,
            mut commands: Commands
        ){
            for (
                entity, 
                InstanceBuffer(instance, _), 
                VertexInstanceBuffer(vertex), 
                IndirectBuffer(indirect),
                VoteScanBuffers { vote, groups, config, ..}
            ) in query.iter(){
                commands.entity(entity).insert(Self { 
                    instance: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Instance Buffer Readback"), 
                        size: instance.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }), 
                    vote: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Vote Buffer Readback"), 
                        size: vote.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    groups: (
                        render_device.create_buffer(&BufferDescriptor { 
                            label: Some("Vote Scan Group 1 Buffer Readback"), 
                            size: groups.0.size(), 
                            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                            mapped_at_creation: false 
                        }), 
                        render_device.create_buffer(&BufferDescriptor { 
                            label: Some("Vote Scan Group 2 Buffer Readback"), 
                            size: groups.1.size(), 
                            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                            mapped_at_creation: false 
                        }),
                    ),
                    indirect: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Indirect Buffer Readback"), 
                        size: indirect.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    vertex: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Vertex Buffer Readback"), 
                        size: vertex.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    config: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Config Buffer Readback"), 
                        size: config.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                });
            }
        }
        fn print_readback(
            query: Query<&ReadbackVoteScanBuffers>,
            render_device: Res<RenderDevice>
        ){
            for ReadbackVoteScanBuffers { 
                instance, vote, groups, indirect, vertex, config 
            } in query.iter(){
                readback_buffer::<CornData>("Instance Buffer: ".to_string(), instance, render_device.as_ref());
                readback_buffer::<[u32; 2]>("vote Buffer: ".to_string(), vote, render_device.as_ref());
                readback_buffer::<[u32; LOD_COUNT as usize]>("Group 1 Buffer: ".to_string(), &groups.0, render_device.as_ref());
                readback_buffer::<[u32; LOD_COUNT as usize]>("Group 2 Buffer: ".to_string(), &groups.1, render_device.as_ref());
                readback_buffer::<[u32; 5]>("Indirect Buffer: ".to_string(), indirect, render_device.as_ref());
                readback_buffer::<Mat4>("Vertex Buffer: ".to_string(), vertex, render_device.as_ref());
                readback_buffer::<ConfigData>("Config Buffer: ".to_string(), config, render_device.as_ref());
            }
        }
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash, RenderLabel)]
    pub struct ReadbackVoteScanStage;
    #[derive(Default, Debug, Clone)]
    pub struct ReadbackVoteScanNode{
        pub ready_entities: Vec<Entity>
    }
    impl bevy::render::render_graph::Node for ReadbackVoteScanNode{
        fn update(&mut self, world: &mut World) {
            let mut query = world.query_filtered::<Entity, (
                With<ReadbackVoteScanBuffers>, With<InstanceBuffer>, With<IndirectBuffer>, With<VertexInstanceBuffer>, With<VoteScanBuffers>
            )>();
            self.ready_entities = query.iter(world).collect();
        }
        fn run<'w>(
            &self,
            _graph: &mut bevy::render::render_graph::RenderGraphContext,
            render_context: &mut bevy::render::renderer::RenderContext<'w>,
            world: &'w World,
        ) -> Result<(), bevy::render::render_graph::NodeRunError> {
            for entity in self.ready_entities.iter(){
                let Some(InstanceBuffer(instance_src, _)) = world.get::<InstanceBuffer>(*entity) else {continue;};
                let Some(IndirectBuffer(indirect_src)) = world.get::<IndirectBuffer>(*entity) else {continue;};
                let Some(VertexInstanceBuffer(vertex_src)) = world.get::<VertexInstanceBuffer>(*entity) else {continue;};
                let Some(VoteScanBuffers{
                    vote: vote_src, groups: (group1_src, group2_src), config: config_src, ..
                }) = world.get::<VoteScanBuffers>(*entity) else {continue;};

                let Some(ReadbackVoteScanBuffers { 
                    instance, vote, groups: (group1, group2), indirect, vertex, config 
                }) = world.get::<ReadbackVoteScanBuffers>(*entity) else {continue;};

                render_context.command_encoder().copy_buffer_to_buffer(
                    instance_src, 0, 
                    instance, 0, 
                    instance.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    indirect_src, 0, 
                    indirect, 0, 
                    indirect.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    vertex_src, 0, 
                    vertex, 0, 
                    vertex.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    group1_src, 0, 
                    group1, 0, 
                    group1.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    group2_src, 0, 
                    group2, 0, 
                    group2.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    vote_src, 0, 
                    vote, 0, 
                    vote.size()
                );
                render_context.command_encoder().copy_buffer_to_buffer(
                    config_src, 0, 
                    config, 0, 
                    config.size()
                );
            }
            Ok(())
        }
    }

    pub struct ReadbackPlugin;
    impl Plugin for ReadbackPlugin{
        fn build(&self, app: &mut App) {
            app
                .register_type::<ReadbackVoteScan>()
                .add_plugins(ExtractComponentPlugin::<ReadbackVoteScan>::default())
            .sub_app_mut(RenderApp)
                .add_systems(Render, (
                    ReadbackVoteScanBuffers::create_buffers.in_set(RenderSet::PrepareResources),
                    ReadbackVoteScanBuffers::print_readback.in_set(RenderSet::Cleanup)
                ));
            let mut render_graph = app.sub_app_mut(RenderApp)
                .world_mut().resource_mut::<RenderGraph>();
            let graph = render_graph.sub_graph_mut(Core3d);
            graph.add_node(ReadbackVoteScanStage, ReadbackVoteScanNode::default());
            graph.add_node_edge(VoteScanStage, ReadbackVoteScanStage);
        }
    }
}
