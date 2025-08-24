pub mod shader;
pub mod scan;
pub mod simple;
pub mod image;

use bevy::{prelude::*, render::{extract_component::{ExtractComponent, ExtractComponentPlugin}, renderer::RenderDevice, Render, RenderApp, RenderSet}};
use bytemuck::{Pod, Zeroable};
use image::ImageInitPlugin;
use shader::CornInitShaderPlugin;
use simple::SimpleInitPlugin;
use scan::CornStoredScanPlugin;
use super::buffer::{CornData, InstanceBuffer, VertexInstanceBuffer};

pub mod prelude{
    pub use super::InitialCornData;
    pub use super::image::{ImageCarvedSettings, ImageCarvedHexagonalSettings};
    pub use super::simple::{SimpleInitSettings, SimpleHexagonalSettings};
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct CornInitShaderSettings{
    origin: Vec3,
    resolution_width: u32,
    height_range: Vec2,
    step_size: Vec2,
    random_settings: Vec2,
    uv_scale: Vec2
}

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
pub struct InitialCornData(pub Vec<CornData>);
impl InitialCornData{
    pub fn upload_data(
        query: Query<(Entity, &Self), Without<InstanceBuffer>>,
        mut commands: Commands,
        render_device: Res<RenderDevice>
    ){
        for(entity, InitialCornData(data)) in query.iter(){
            commands.entity(entity).insert((
                InstanceBuffer::create_buffer_with_data(
                    "Corn Field Instance Buffer".to_string(), 
                    render_device.as_ref(), 
                    bytemuck::cast_slice::<CornData, u8>(data.as_slice())
                ),
                VertexInstanceBuffer::create_buffer(
                    "Corn Field Vertex Instance Buffer".to_string(), 
                    data.len() as u64, 
                    render_device.as_ref()
                )
            ));
        }
    }
}

/// Global Code for the init shader invocations
#[derive(Debug, Default, Clone)]
pub struct CornInitializationPlugin;
impl Plugin for CornInitializationPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<InitialCornData>()
            .add_plugins(ExtractComponentPlugin::<InitialCornData>::default())
            .add_plugins((CornInitShaderPlugin, CornStoredScanPlugin))
        .sub_app_mut(RenderApp)
            .add_systems(Render, InitialCornData::upload_data.in_set(RenderSet::PrepareResources));
        // Init Shader Plugins
        app.add_plugins((SimpleInitPlugin, ImageInitPlugin));
        // Readback plugin
        #[cfg(debug_assertions)]
        app.add_plugins(readback::ReadbackPlugin);
    }
}

/*
    Readback Buffer
*/
#[cfg(debug_assertions)]
pub mod readback{
    use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
    use bevy::{prelude::*, render::{
        render_graph::{RenderGraph, RenderLabel}, 
        render_resource::Buffer, renderer::RenderDevice, 
        sync_component::SyncComponentPlugin, sync_world::{MainEntity, RenderEntity}, 
        Extract, MainWorld, Render, RenderApp, RenderSet
    }};
    use wgpu::{BufferUsages, Maintain, MapMode};
    use wgpu_types::BufferDescriptor;
    use crate::ecs::corn::buffer::{CornData, InstanceBuffer};
    
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)]
    pub struct ReadbackInit(pub Vec<CornData>);
    impl ReadbackInit{
        /// Copies readback data to the main world entity
        fn copy_data(
            render: Query<(MainEntity, &Self), Changed<Self>>,
            mut main_world: ResMut<MainWorld>
        ){
            for (entity, data) in render.iter(){
                if let Some(comp) = main_world.entity_mut(entity).get_mut::<Self>(){
                    comp.into_inner().0 = data.0.clone();
                }
            }
        }
        /// Attaches this component to any field in the render app that needs it. Does not copy the data over
        fn extract_component(
            main: Extract<Query<RenderEntity, With<ReadbackInit>>>,
            render: Query<MainEntity, Without<ReadbackInit>>,
            mut commands: Commands
        ){
            let mut batch = vec![];
            for entity in main.iter(){
                if render.contains(entity) {batch.push((entity, Self::default()));}
            }
            commands.insert_batch(batch);
        }
    }

    #[derive(Debug, Clone, Component)]
    pub struct ReadbackInitBuffer(pub Buffer);
    impl ReadbackInitBuffer{
        /// System which sets up a readback every time the instance buffer changes
        fn on_change(
            query: Query<(Entity, &InstanceBuffer), (With<ReadbackInit>, Changed<InstanceBuffer>)>,
            mut commands: Commands,
            render_device: Res<RenderDevice>
        ){
            let mut batch = vec![];
            for (entity, buffer) in query.iter(){
                let dst_buffer = render_device.create_buffer(&BufferDescriptor{
                    label: Some("Init Readback Buffer"),
                    size: buffer.buffer.size(),
                    usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                    mapped_at_creation: false
                });
                batch.push((entity, Self(dst_buffer)));
            }
            commands.insert_batch(batch);
        }
        /// System which reads the data and sends it to the readback component
        fn finish_readback(
            mut query: Query<(Entity, &ReadbackInitBuffer, &mut ReadbackInit)>,
            mut commands: Commands,
            render_device: Res<RenderDevice>
        ){
            for (entity, ReadbackInitBuffer(buffer), comp) in query.iter_mut(){
                let slice = buffer.slice(..);
                let flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
                let flag_captured = flag.clone();
                slice.map_async(MapMode::Read, move |v|{
                    if v.is_ok() {
                        flag_captured.store(true, Ordering::Relaxed);
                    }
                });
                render_device.poll(Maintain::Wait);
                if flag.load(Ordering::Relaxed){
                    let raw = slice.get_mapped_range();
                    let data = bytemuck::cast_slice::<u8, CornData>(&raw).to_vec();
                    debug!(?data);
                    comp.into_inner().0 = data;
                }
                commands.entity(entity).remove::<Self>();
            }
        }
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash, RenderLabel)]
    pub struct ReadbackInitStage;
    #[derive(Default, Debug, Clone)]
    pub struct ReadbackInitNode{
        pub ready_entities: Vec<Entity>
    }
    impl bevy::render::render_graph::Node for ReadbackInitNode{
        fn update(&mut self, world: &mut World) {
            let mut query = world.query_filtered::<Entity, (With<ReadbackInitBuffer>, With<InstanceBuffer>)>();
            self.ready_entities = query.iter(world).collect();
        }
        fn run<'w>(
            &self,
            _graph: &mut bevy::render::render_graph::RenderGraphContext,
            render_context: &mut bevy::render::renderer::RenderContext<'w>,
            world: &'w World,
        ) -> Result<(), bevy::render::render_graph::NodeRunError> {
            for entity in self.ready_entities.iter(){
                let Some(InstanceBuffer{buffer, ..}) = world.get::<InstanceBuffer>(*entity) else {continue;};
                let Some(ReadbackInitBuffer(readback)) = world.get::<ReadbackInitBuffer>(*entity) else {continue;};
                render_context.command_encoder().copy_buffer_to_buffer(
                    buffer, 0, 
                    readback, 0, 
                    buffer.size()
                );
            }
            Ok(())
        }
    }

    pub struct ReadbackPlugin;
    impl Plugin for ReadbackPlugin{
        fn build(&self, app: &mut App) {
            app
                .register_type::<ReadbackInit>()
                .add_plugins(SyncComponentPlugin::<ReadbackInit>::default())
            .sub_app_mut(RenderApp)
                .add_systems(ExtractSchedule, (ReadbackInit::copy_data, ReadbackInit::extract_component))
                .add_systems(Render, (
                    ReadbackInitBuffer::on_change.in_set(RenderSet::PrepareResources),
                    ReadbackInitBuffer::finish_readback.in_set(RenderSet::Cleanup)
                ))
                .world_mut().resource_mut::<RenderGraph>()
                    .add_node(ReadbackInitStage, ReadbackInitNode::default());
        }
    }
}

