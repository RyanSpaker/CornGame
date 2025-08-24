use std::sync::atomic::Ordering;
use bevy::{
    asset::AsAssetId, core_pipeline::core_3d::graph::Core3d, ecs::system::lifetimeless::Read, pbr::graph::NodePbr, prelude::*, 
    render::{
        mesh::allocator::MeshAllocator, view::ExtractedView, Render, RenderApp, RenderSet,
        extract_component::{ExtractComponent, ExtractComponentPlugin}, 
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel}, 
        render_resource::{BindGroup, BindGroupLayout, Buffer, CachedComputePipelineId, ComputePipelineDescriptor, PipelineCache, ShaderType}, 
        renderer::{RenderContext, RenderDevice, RenderQueue},
    }
};
use bytemuck::{Pod, Zeroable};
use wgpu::{BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, BufferDescriptor, BufferUsages, ComputePassDescriptor, ShaderStages};
use crate::{ecs::{cameras::MainCamera, corn::{asset::{CornLodInfo, CornMesh, CornModel, CornModelIndirectBuffer}, buffer::{IndirectBuffer, InstanceBuffer, VertexInstanceBuffer}, cutoffs::LodCutoffBuffer}}};

/// Tag component which signifies when corn fields use the storedscan pipeline. Added by init shader invocation code
#[derive(Debug, Default, Clone, Component)]
pub struct UseStoredScanPipeline;

/// Component which holds the vote buffer used by the vote scan process, and whose size changes depending on instance buffer's size
#[derive(Debug, Component)]
pub struct StoredScanVoteBuffer(pub Buffer);
impl StoredScanVoteBuffer{
    /// System which creates vote buffers when needed
    pub fn create_buffers(
        query: Query<(Entity, &InstanceBuffer, Option<&Self>), (With<UseStoredScanPipeline>, Or<(Without<Self>, Changed<InstanceBuffer>)>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, instance, vote) in query.iter(){
            if vote.is_some_and(|v| v.0.size() == instance.instance_count*8) {continue;}
            let buffer = render_device.create_buffer(&BufferDescriptor{
                label: Some("Corn Field Vote Buffer"),
                size: instance.instance_count*8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                mapped_at_creation: false
            });
            commands.entity(entity).insert(Self(buffer));
        }
    }
}

/// Component which holds the two group buffers whose size depends on instance buffers size and the lod count
#[derive(Debug, Component)]
pub struct StoredScanGroupBuffers(pub Buffer, pub Buffer);
impl StoredScanGroupBuffers{
    /// System which creates group buffers when needed
    pub fn create_buffers(
        query: Query<
            (Entity, &InstanceBuffer, &CornLodInfo, Option<&Self>), 
            (With<UseStoredScanPipeline>, Or<(Without<Self>, Changed<InstanceBuffer>, Changed<CornLodInfo>)>)
        >,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, instance, lod_info, group) in query.iter(){
            let g1_size = instance.instance_count.div_ceil(256); let g2_size = g1_size.div_ceil(256);
            let byte_count: u64 = lod_info.0.len() as u64*4;
            if group.is_some_and(|g| g.0.size() == g1_size*byte_count) {continue;}
            commands.entity(entity).insert(Self(
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Field Group 1 Buffer"), 
                    size: g1_size*byte_count,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false
                }),
                render_device.create_buffer(&BufferDescriptor { 
                    label: Some("Corn Field Group 2 Buffer"), 
                    size: g2_size*byte_count,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                    mapped_at_creation: false 
                }),
            ));
        }
    }
}

/// Struct mirroring the config data needed for the vote-scan-compact shaders. Passed in as a buffer
#[derive(Clone, Copy, Default, Debug, Zeroable, Pod, ShaderType, PartialEq, Reflect)]
#[repr(C)]
pub struct ConfigData{
    pub field_to_world: Mat4,
    pub field_to_clip: Mat4,
    pub cam_pos_field: Vec4,
    pub lod_count: u32,
    pub index_offset: u32,
    pub vertex_offset: u32,
    pub _padding: u32
}

/// Custom component containing the global transform of the corn field.
#[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
pub struct CornFieldTransform(pub Transform);
impl ExtractComponent for CornFieldTransform{
    type Out = Self;
    type QueryData = Read<GlobalTransform>;
    type QueryFilter = ();
    fn extract_component(item: bevy::ecs::query::QueryItem<'_, Self::QueryData>) -> Option<Self::Out> {Some(Self(item.compute_transform()))}
}

/// Component which holds the config buffer for the vote scan prepass
#[derive(Debug, Component)]
pub struct StoredScanConfigBuffer(pub Buffer);
impl StoredScanConfigBuffer{
    /// System which creates the necessary config buffers
    pub fn create_buffers(
        query: Query<Entity, (With<UseStoredScanPipeline>, Without<Self>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for entity in query.iter(){
            commands.entity(entity).insert(Self(render_device.create_buffer(&BufferDescriptor{
                label: Some("Corn Field Scan Prepass Config Buffer"), 
                size: size_of::<ConfigData>() as u64, 
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC, 
                mapped_at_creation: false
            })));
        }
    }
    /// Computes and writes config data to config buffers every frame
    pub fn update_buffers(
        query: Query<(&CornFieldTransform, &Self, &CornLodInfo, &CornModel)>,
        models: Query<&CornMesh>,
        render_queue: Res<RenderQueue>,
        camera: Single<&ExtractedView, With<MainCamera>>,
        mesh_allocator: Res<MeshAllocator>
    ){
        let cam_pos = camera.world_from_view.translation().extend(1.0);
        let w2c = camera.clip_from_view*camera.world_from_view.compute_matrix().inverse();

        for (transform, buffers, lod_info, model) in query.iter(){
            let field_to_world = transform.0.compute_matrix();
            let field_to_clip = w2c*field_to_world;
            let cam_pos_field = field_to_world.inverse().mul_vec4(cam_pos);
            let lod_count = lod_info.0.len() as u32;

            let Ok(mesh) = models.get(model.0) else {continue;};
            let Some(vertex) = mesh_allocator.mesh_vertex_slice(&mesh.as_asset_id()) else {continue;};
            let Some(index) = mesh_allocator.mesh_index_slice(&mesh.as_asset_id()) else {continue;};
            let index_offset = index.range.start;
            let vertex_offset = vertex.range.start;

            render_queue.write_buffer(
                &buffers.0, 
                0, 
                bytemuck::cast_slice::<ConfigData, u8>(&[ConfigData{
                    field_to_clip, field_to_world, cam_pos_field, 
                    lod_count, index_offset, vertex_offset, 
                    ..Default::default()
                }])
            );
        }
    }
}

/// Component which holds the bind group as well as the dispatch count
#[derive(Debug, Clone, Component)]
pub struct StoredScanBindGroup(pub BindGroup, pub [u32; 4]);
impl StoredScanBindGroup{
    /// System which creates bindgroup when needed and on buffer changes
    pub fn create_bindgroups(
        // All corn fields with the required buffers, without a bind group or a with buffer that has changed.
        query: Query<(
            Entity, 
            &InstanceBuffer, &VertexInstanceBuffer, 
            &StoredScanVoteBuffer, &StoredScanGroupBuffers, &StoredScanConfigBuffer, 
            &IndirectBuffer, &LodCutoffBuffer
        ), (
            With<UseStoredScanPipeline>, Or<(
                Without<Self>,
                Changed<InstanceBuffer>, Changed<VertexInstanceBuffer>,
                Changed<StoredScanVoteBuffer>, Changed<StoredScanGroupBuffers>, Changed<StoredScanConfigBuffer>,
                Changed<IndirectBuffer>, Changed<LodCutoffBuffer>,
            )>
        )>,
        pipeline: Res<StoredScanPipeline>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, instance, vib, vote, group, config, indirect, cutoffs) in query.iter(){
            let bindgroup = render_device.create_bind_group(
                Some("Corn Field Scan Prepass Bind Group"), 
                &pipeline.layout, 
                &[
                    BindGroupEntry{binding: 0, resource: instance.buffer.as_entire_binding()},
                    BindGroupEntry{binding: 1, resource: vote.0.as_entire_binding()},
                    BindGroupEntry{binding: 2, resource: group.0.as_entire_binding()},
                    BindGroupEntry{binding: 3, resource: group.1.as_entire_binding()},
                    BindGroupEntry{binding: 4, resource: indirect.buffer.as_entire_binding()},
                    BindGroupEntry{binding: 5, resource: vib.buffer.as_entire_binding()},
                    BindGroupEntry{binding: 6, resource: config.0.as_entire_binding()},
                    BindGroupEntry{binding: 7, resource: cutoffs.0.as_entire_binding()}
                ]
            );
            let a = instance.instance_count.div_ceil(256); let b = a.div_ceil(256); let c = b.div_ceil(256);
            let dispatch = [a as u32, b as u32,c as u32, a as u32];
            commands.entity(entity).insert(Self(bindgroup, dispatch));
        }
    }
}

/// Pipeline resources for the 4 vote-scan-compact shaders
#[derive(Debug, Clone, Resource)]
pub struct StoredScanPipeline{
    pub layout: BindGroupLayout,
    pub pipelines: Vec<CachedComputePipelineId>,
    pub shader: Handle<Shader>
}
impl FromWorld for StoredScanPipeline{
    fn from_world(world: &mut World) -> Self {
        let shader: Handle<Shader> = world.resource::<AssetServer>().load("shaders/corn/scan_prepass.wgsl");
        let layout = world.resource::<RenderDevice>().create_bind_group_layout(
            Some("Stored Scan BindGroup Layout"), 
            [false, false, false, false, false, false, true, false].into_iter().enumerate()
                .map(|(binding, uniform)| BindGroupLayoutEntry{
                    binding: binding as u32, 
                    visibility: ShaderStages::COMPUTE,
                    count: None,
                    ty: BindingType::Buffer { 
                        ty: if uniform {BufferBindingType::Uniform} else {BufferBindingType::Storage { read_only: binding==0 || binding == 7 }}, 
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
                push_constant_ranges: vec![],
                shader: shader.clone(),
                shader_defs: vec![],
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

/// Render Graph Label for Init Operations
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, RenderLabel)]
struct StoredScanStage;
/// This is the render graph node which executes the Scan Prepass
#[derive(Debug, Default, Clone)]
pub struct StoredScanNode{
    ready_entities: Vec<Entity>
}
impl Node for StoredScanNode{
    fn update(&mut self, world: &mut World) {
        let mut query: _ = world.query_filtered::<Entity, (With<StoredScanBindGroup>, With<IndirectBuffer>, With<CornModel>)>();
        self.ready_entities = query.iter(world).collect();
    }
    fn run<'w>(
        &self, _graph: &mut RenderGraphContext, render_context: &mut RenderContext<'w>, world: &'w World,
    ) -> Result<(), NodeRunError> {
        // Get pipelines
        let pipeline_ids = world.resource::<StoredScanPipeline>();
        let cache = world.resource::<PipelineCache>();
        let mut pipelines = vec![];
        for pipeline in pipeline_ids.pipelines.iter(){
            let Some(pipeline) = cache.get_compute_pipeline(*pipeline) else {return Ok(());};
            pipelines.push(pipeline);
        }
        let mut valid_entities = vec![];
        // Copy indirect Buffers
        for entity in self.ready_entities.iter(){
            let Some(IndirectBuffer {buffer, ..}) = world.get::<IndirectBuffer>(*entity) else {continue;};
            let Some(CornModel(model)) = world.get::<CornModel>(*entity) else {continue;};
            let Some(CornModelIndirectBuffer(src, _)) = world.get::<CornModelIndirectBuffer>(*model) else {continue;};
            let Some(StoredScanBindGroup(bindgroup, dispatch)) = world.get::<StoredScanBindGroup>(*entity) else {continue;};
            let Some(vib) = world.get::<VertexInstanceBuffer>(*entity) else {continue;};
            render_context.command_encoder().copy_buffer_to_buffer(src, 0, buffer, 0, src.size());
            valid_entities.push((bindgroup, dispatch, vib));
        }
        // Start Compute Pass
        let mut compute_pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor { 
            label: Some("Stored Scan Compute Pass"), timestamp_writes: None 
        });
        // Vote
        compute_pass.set_pipeline(pipelines[0]);
        for (bindgroup, dispatch, _) in valid_entities.iter(){
            compute_pass.set_bind_group(0, *bindgroup, &[]);
            compute_pass.dispatch_workgroups(dispatch[0], 1, 1);
        }
        // Group 1
        compute_pass.set_pipeline(pipelines[1]);
        for (bindgroup, dispatch, _) in valid_entities.iter(){
            compute_pass.set_bind_group(0, *bindgroup, &[]);
            compute_pass.dispatch_workgroups(dispatch[1], 1, 1);
        }
        // Group 2
        compute_pass.set_pipeline(pipelines[2]);
        for (bindgroup, dispatch, _) in valid_entities.iter(){
            compute_pass.set_bind_group(0, *bindgroup, &[]);
            compute_pass.dispatch_workgroups(dispatch[2], 1, 1);
        }
        // Compact
        compute_pass.set_pipeline(pipelines[3]);
        for (bindgroup, dispatch, _) in valid_entities.iter(){
            compute_pass.set_bind_group(0, *bindgroup, &[]);
            compute_pass.dispatch_workgroups(dispatch[3], 1, 1);
        }
        // Set vib to valid
        for (_, _, vib) in valid_entities.into_iter(){
            vib.ready.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

pub struct CornStoredScanPlugin;
impl Plugin for CornStoredScanPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_plugins(ExtractComponentPlugin::<CornFieldTransform>::default())
        .sub_app_mut(RenderApp)
            .add_systems(Render, (
                StoredScanVoteBuffer::create_buffers,
                StoredScanGroupBuffers::create_buffers,
                StoredScanConfigBuffer::create_buffers
            ).in_set(RenderSet::PrepareResources))
            .add_systems(Render, StoredScanConfigBuffer::update_buffers
                .after(StoredScanConfigBuffer::create_buffers)
                .in_set(RenderSet::Prepare)
            )
            .add_systems(Render, StoredScanBindGroup::create_bindgroups.in_set(RenderSet::PrepareBindGroups));
        // Add Scan Node to RenderGraph
        let mut render_graph = app.sub_app_mut(RenderApp)
            .world_mut().resource_mut::<RenderGraph>();
        let graph = render_graph.sub_graph_mut(Core3d);
        graph.add_node(StoredScanStage, StoredScanNode::default());
        graph.add_node_edge(StoredScanStage, NodePbr::EarlyShadowPass);
        // Readback
        #[cfg(debug_assertions)]
        app.add_plugins(readback::ReadbackPlugin);
    }
    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<StoredScanPipeline>();
    }
}

#[cfg(debug_assertions)]
pub mod readback{
    use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
    use bevy::{core_pipeline::core_3d::graph::Core3d, ecs::{query::QueryData, system::lifetimeless::Read}, prelude::*, render::{ 
        render_graph::{RenderGraph, RenderLabel}, render_resource::Buffer, renderer::RenderDevice, sync_component::SyncComponentPlugin, sync_world::{MainEntity, RenderEntity}, Extract, MainWorld, Render, RenderApp, RenderSet
    }};
    use bytemuck::Pod;
    use wgpu::{BufferUsages, Maintain, MapMode};
    use wgpu_types::BufferDescriptor;
    use crate::ecs::corn::{asset::{CornLodInfo, CornModel, CornModelIndirectBuffer}, buffer::{CornData, IndirectBuffer, InstanceBuffer, VertexInstanceBuffer}, cutoffs::LodCutoffBuffer};
    use super::{ConfigData, StoredScanBindGroup, StoredScanConfigBuffer, StoredScanGroupBuffers, StoredScanStage, StoredScanVoteBuffer, UseStoredScanPipeline};
    
    pub fn readback_buffer<T: Pod>(buffer: &Buffer, render_device: &RenderDevice) -> Vec<T>{
        let slice = buffer.slice(..);
        let flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let flag_captured = flag.clone();
        slice.map_async(MapMode::Read, move |v|{
            if v.is_ok() {flag_captured.store(true, Ordering::Relaxed);}
        });
        render_device.poll(Maintain::Wait);
        let mut data = vec![];
        if flag.load(Ordering::Relaxed) {
            let raw = slice.get_mapped_range();
            data = bytemuck::cast_slice::<u8, T>(&raw).to_vec();
        }
        buffer.unmap(); 
        data
    }

    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)]
    pub struct ReadbackStoredScan{
        pub instances: Vec<CornData>,
        pub vertex_instances: Vec<Mat4>,
        pub indirect: Vec<[u32; 5]>,
        pub model_indirect: Vec<[u32; 5]>,
        pub lod_count: usize,
        pub vote: Vec<[u32; 2]>,
        pub group1: Vec<Vec<u32>>,
        pub group2: Vec<Vec<u32>>,
        pub config: ConfigData,
        pub cutoffs: Vec<f32>
    }
    impl ReadbackStoredScan{
        /// Copies readback data to the main world entity
        fn copy_data(
            render: Query<(MainEntity, &Self), Changed<Self>>,
            mut main_world: ResMut<MainWorld>
        ){
            for (entity, data) in render.iter(){
                if let Some(comp) = main_world.entity_mut(entity).get_mut::<Self>(){
                    *comp.into_inner() = data.clone();
                }
            }
        }
        /// Attaches this component to any field in the render app that needs it. Does not copy the data over
        fn extract_component(
            main: Extract<Query<RenderEntity, With<Self>>>,
            render: Query<MainEntity, Without<Self>>,
            mut commands: Commands
        ){
            let mut batch = vec![];
            for entity in main.iter(){
                if render.contains(entity) {batch.push((entity, Self::default()));}
            }
            commands.insert_batch(batch);
        }
    
    }

    #[derive(QueryData)]
    pub struct StoredScanBufferQuery{
        instance: Read<InstanceBuffer>,
        vib: Read<VertexInstanceBuffer>,
        ind: Read<IndirectBuffer>,
        model: Read<CornModel>,
        vote: Read<StoredScanVoteBuffer>,
        group: Read<StoredScanGroupBuffers>,
        config: Read<StoredScanConfigBuffer>,
        cutoffs: Read<LodCutoffBuffer>
    }

    #[derive(Debug, Clone, Component)]
    pub struct ReadbackStoredScanBuffers{
        instance: Buffer,
        vote: Buffer,
        groups: (Buffer, Buffer),
        indirect: Buffer,
        ind_src: Buffer,
        vertex: Buffer,
        config: Buffer,
        cutoffs: Buffer
    }
    impl ReadbackStoredScanBuffers{
        /// Creates the readback buffers necessary
        fn create_buffers(
            query: Query<(Entity, StoredScanBufferQuery), (With<ReadbackStoredScan>, Without<Self>, With<UseStoredScanPipeline>, With<StoredScanBindGroup>)>,
            models: Query<&CornModelIndirectBuffer>,
            render_device: Res<RenderDevice>,
            mut commands: Commands
        ){
            for (entity, StoredScanBufferQueryItem{
                instance, vib, ind, model, vote, group, config, cutoffs
            }) in query.iter(){
                let Ok(ind_src) = models.get(model.0) else {continue;};

                commands.entity(entity).insert(Self{ 
                    instance: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Instance Buffer Readback"), 
                        size: instance.buffer.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }), 
                    vote: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Vote Buffer Readback"), 
                        size: vote.0.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    groups: (
                        render_device.create_buffer(&BufferDescriptor { 
                            label: Some("Vote Scan Group 1 Buffer Readback"), 
                            size: group.0.size(), 
                            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                            mapped_at_creation: false 
                        }), 
                        render_device.create_buffer(&BufferDescriptor { 
                            label: Some("Vote Scan Group 2 Buffer Readback"), 
                            size: group.1.size(), 
                            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                            mapped_at_creation: false 
                        }),
                    ),
                    indirect: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Indirect Buffer Readback"), 
                        size: ind.buffer.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    ind_src: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Indirect Src Buffer Readback"), 
                        size: ind_src.0.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    vertex: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Vertex Buffer Readback"), 
                        size: vib.buffer.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    config: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Config Buffer Readback"), 
                        size: config.0.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                    cutoffs: render_device.create_buffer(&BufferDescriptor { 
                        label: Some("Vote Scan Cutoffs Buffer Readback"), 
                        size: cutoffs.0.size(), 
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                        mapped_at_creation: false 
                    }),
                });
            }
        }
        /// Reads the data from the buffers and copies it into the component
        fn copy_data(
            mut query: Query<(&Self, &CornLodInfo, &mut ReadbackStoredScan)>,
            render_device: Res<RenderDevice>
        ){
            for (buffers, lod_info, mut dst) in query.iter_mut(){
                dst.lod_count = lod_info.0.len();
                dst.instances = readback_buffer::<CornData>(&buffers.instance, render_device.as_ref());
                dst.vertex_instances = readback_buffer::<Mat4>(&buffers.vertex, render_device.as_ref());
                dst.vote = readback_buffer::<[u32; 2]>(&buffers.vote, render_device.as_ref());
                dst.group1 = readback_buffer::<u32>(&buffers.groups.0, render_device.as_ref())
                    .chunks(dst.lod_count).map(|chunk| chunk.to_vec()).collect();
                dst.group2 = readback_buffer::<u32>(&buffers.groups.1, render_device.as_ref())
                    .chunks(dst.lod_count).map(|chunk| chunk.to_vec()).collect();
                dst.indirect = readback_buffer::<[u32; 5]>(&buffers.indirect, render_device.as_ref());
                dst.model_indirect = readback_buffer::<[u32; 5]>(&buffers.ind_src, render_device.as_ref());
                dst.config = readback_buffer::<ConfigData>(&buffers.config, render_device.as_ref()).pop().unwrap_or_default();
                dst.cutoffs = readback_buffer::<f32>(&buffers.cutoffs, render_device.as_ref());
                debug!(scan_data = ?dst);
            }
        }
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash, RenderLabel)]
    pub struct ReadbackStoredScanStage;
    #[derive(Default, Debug, Clone)]
    pub struct ReadbackStoredScanNode{
        pub ready_entities: Vec<Entity>
    }
    impl bevy::render::render_graph::Node for ReadbackStoredScanNode{
        fn update(&mut self, world: &mut World) {
            let mut query = world.query_filtered::<Entity, With<ReadbackStoredScanBuffers>>();
            self.ready_entities = query.iter(world).collect();
        }
        fn run<'w>(
            &self,
            _graph: &mut bevy::render::render_graph::RenderGraphContext,
            render_context: &mut bevy::render::renderer::RenderContext<'w>,
            world: &'w World,
        ) -> Result<(), bevy::render::render_graph::NodeRunError> {
            for entity in self.ready_entities.iter(){
                let Some(instance) = world.get::<InstanceBuffer>(*entity) else {continue;};
                let Some(vote) = world.get::<StoredScanVoteBuffer>(*entity) else {continue;};
                let Some(groups) = world.get::<StoredScanGroupBuffers>(*entity) else {continue;};
                let Some(indirect) = world.get::<IndirectBuffer>(*entity) else {continue;};
                let Some(model) = world.get::<CornModel>(*entity) else {continue;};
                let Some(ind_src) = world.get::<CornModelIndirectBuffer>(model.0) else {continue;};
                let Some(vertex) = world.get::<VertexInstanceBuffer>(*entity) else {continue;};
                let Some(config) = world.get::<StoredScanConfigBuffer>(*entity) else {continue;};
                let Some(cutoffs) = world.get::<LodCutoffBuffer>(*entity) else {continue;};
                let Some(readback) = world.get::<ReadbackStoredScanBuffers>(*entity) else {continue;};

                render_context.command_encoder().copy_buffer_to_buffer(
                    &instance.buffer, 0, 
                    &readback.instance, 0, 
                    instance.buffer.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &vote.0, 0, 
                    &readback.vote, 0, 
                    vote.0.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &groups.0, 0, 
                    &readback.groups.0, 0, 
                    groups.0.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &groups.1, 0, 
                    &readback.groups.1, 0, 
                    groups.1.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &indirect.buffer, 0, 
                    &readback.indirect, 0, 
                    indirect.buffer.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &ind_src.0, 0, 
                    &readback.ind_src, 0, 
                    ind_src.0.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &vertex.buffer, 0, 
                    &readback.vertex, 0, 
                    vertex.buffer.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &config.0, 0, 
                    &readback.config, 0, 
                    config.0.size()
                );
                 render_context.command_encoder().copy_buffer_to_buffer(
                    &cutoffs.0, 0, 
                    &readback.cutoffs, 0, 
                    cutoffs.0.size()
                );
            }
            Ok(())
        }
    }

    pub struct ReadbackPlugin;
    impl Plugin for ReadbackPlugin{
        fn build(&self, app: &mut App) {
            app
                .register_type::<ReadbackStoredScan>()
                .add_plugins(SyncComponentPlugin::<ReadbackStoredScan>::default())
            .sub_app_mut(RenderApp)
                .add_systems(ExtractSchedule, (ReadbackStoredScan::extract_component, ReadbackStoredScan::copy_data))
                .add_systems(Render, (
                    ReadbackStoredScanBuffers::create_buffers.in_set(RenderSet::PrepareResources),
                    ReadbackStoredScanBuffers::copy_data.in_set(RenderSet::Cleanup)
                ));
            let mut render_graph = app.sub_app_mut(RenderApp)
                .world_mut().resource_mut::<RenderGraph>();
            let graph = render_graph.sub_graph_mut(Core3d);
            graph.add_node(ReadbackStoredScanStage, ReadbackStoredScanNode::default());
            graph.add_node_edge(StoredScanStage, ReadbackStoredScanStage);
        }
    }
}

