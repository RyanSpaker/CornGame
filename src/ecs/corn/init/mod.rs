pub mod shader;
pub mod simple;

use bevy::{prelude::*, render::{extract_component::{ExtractComponent, ExtractComponentPlugin}, renderer::RenderDevice, Render, RenderApp, RenderSet}};
use shader::CornInitShaderPlugin;
use simple::SimpleInitPlugin;

use super::{CornData, CornLoaded, InstanceBuffer};

/*
    Load Shader from file into Handle<Shader>
    Create BindGroupLayout per shader
    Queue pipeline creation using shader and bindgrouplayout

    Create GPU resources containing necessary data for shader invocation
    Create Bind groups using resources

    Once pipeline is created, we can proceed
    In a node, Start a compute pass
    Set pipeline, Set bindgroup, invoke sum # of times
*/
#[derive(Debug, Default, Clone, PartialEq, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
pub struct InitialCornData(pub Vec<CornData>);
impl InitialCornData{
    pub fn upload_data(
        query: Query<(Entity, &Self), (Without<CornLoaded>, Without<InstanceBuffer>)>,
        mut commands: Commands,
        render_device: Res<RenderDevice>
    ){
        for(entity, InitialCornData(data)) in query.iter(){
            commands.entity(entity).insert(InstanceBuffer::create_buffer_with_data(
                "Corn Field Instance Buffer".to_string(), 
                render_device.as_ref(), 
                bytemuck::cast_slice::<CornData, u8>(data.as_slice())
            ));
        }
    }
}

/// Global Code for the init shader invocations
#[derive(Debug, Default, Clone)]
pub struct CornInitializationPlugin;
impl Plugin for CornInitializationPlugin{
    fn build(&self, app: &mut App) {
        app.register_type::<InitialCornData>()
            .add_plugins(ExtractComponentPlugin::<InitialCornData>::default())
            .add_plugins(CornInitShaderPlugin)
        .sub_app_mut(RenderApp)
            .add_systems(Render, InitialCornData::upload_data.in_set(RenderSet::PrepareResources));
        // Init Shader Plugins
        app.add_plugins(SimpleInitPlugin);
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
    use bevy::{ecs::event::EventCursor, prelude::*, render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin}, 
        render_graph::{RenderGraph, RenderLabel}, 
        render_resource::Buffer, renderer::RenderDevice, 
        Render, RenderApp, RenderSet
    }};
    use wgpu::{BufferUsages, Maintain, MapMode};
    use wgpu_types::BufferDescriptor;
    use crate::ecs::corn::{CornData, CornLoaded, InstanceBuffer};
    
    #[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Component, ExtractComponent)]
    #[reflect(Component)]
    pub struct ReadbackInit;

    #[derive(Debug, Clone, Component)]
    pub struct ReadbackInitBuffer(pub Buffer);
    impl ReadbackInitBuffer{
        fn start_readback(
            query: Query<(Entity, &InstanceBuffer), (With<CornLoaded>, With<ReadbackInit>, Without<Self>)>,
            render_device: Res<RenderDevice>,
            mut commands: Commands,
            mut event_writer: EventWriter<ReadbackInitEvent>
        ){
            let mut events = vec![];
            for (entity, InstanceBuffer(_, count)) in query.iter(){
                let buffer = render_device.create_buffer(&BufferDescriptor{
                    label: Some("Init Readback Buffer"),
                    size: count*CornData::DATA_SIZE,
                    usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                    mapped_at_creation: false
                });
                commands.entity(entity).insert(Self(buffer));
                events.push(ReadbackInitEvent(entity));
            }
            event_writer.send_batch(events);
        }
        fn finish_readback(
            query: Query<&ReadbackInitBuffer>,
            mut events: EventReader<ReadbackInitEvent>,
            render_device: Res<RenderDevice>
        ){
            for ReadbackInitEvent(entity) in events.read(){
                let Ok(ReadbackInitBuffer(buffer)) = query.get(*entity) else {continue;};
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
                    let raw = buffer
                        .slice(..).get_mapped_range()
                        .iter().map(|v| *v).collect::<Vec<u8>>();
                    let data = bytemuck::cast_slice::<u8, CornData>(raw.as_slice()).to_vec();
                    println!("Corn Field Init Readback:");
                    for corn in data{
                        println!("{:?}", corn);
                    }
                    println!("");
                }
            }
        }
    }

    #[derive(Debug, Clone, Event)]
    pub struct ReadbackInitEvent(pub Entity);

    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash, RenderLabel)]
    pub struct ReadbackInitStage;
    #[derive(Default, Debug, Clone)]
    pub struct ReadbackInitNode{
        pub ready_entities: Vec<Entity>,
        event_cursor: EventCursor<ReadbackInitEvent>
    }
    impl bevy::render::render_graph::Node for ReadbackInitNode{
        fn update(&mut self, world: &mut World) {
            let entities = self.event_cursor.read(world.resource::<Events<ReadbackInitEvent>>())
                .map(|ReadbackInitEvent(entity)| *entity)
                .collect();
            self.ready_entities = entities;
        }
        fn run<'w>(
            &self,
            _graph: &mut bevy::render::render_graph::RenderGraphContext,
            render_context: &mut bevy::render::renderer::RenderContext<'w>,
            world: &'w World,
        ) -> Result<(), bevy::render::render_graph::NodeRunError> {
            for entity in self.ready_entities.iter(){
                let Some(InstanceBuffer(instance, _)) = world.get::<InstanceBuffer>(*entity) else {continue;};
                let Some(ReadbackInitBuffer(readback)) = world.get::<ReadbackInitBuffer>(*entity) else {continue;};
                render_context.command_encoder().copy_buffer_to_buffer(
                    instance, 0, 
                    readback, 0, 
                    instance.size()
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
                .add_plugins(ExtractComponentPlugin::<ReadbackInit>::default())
            .sub_app_mut(RenderApp)
                .add_event::<ReadbackInitEvent>()
                .add_systems(Render, (
                    ReadbackInitBuffer::start_readback.in_set(RenderSet::PrepareResources),
                    ReadbackInitBuffer::finish_readback.in_set(RenderSet::Cleanup)
                ))
                .world_mut().resource_mut::<RenderGraph>()
                    .add_node(ReadbackInitStage, ReadbackInitNode::default());
        }
    }
}

