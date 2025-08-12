use std::sync::atomic::{AtomicBool, Ordering};
use bevy::{prelude::*, render::{
    render_graph::*, render_resource::*, 
    renderer::{RenderContext, RenderDevice}, 
    Render, RenderApp, RenderSet
}};
use crate::{ecs::corn::{buffer::{InstanceBuffer, VertexInstanceBuffer}, shader::*}, util::extract_changed::ExtractChangedComponentPlugin};

use super::scan::UseStoredScanPipeline;

/// Functionality necessary for a init shader.
pub trait AsCornInitShader: Component+Sized+AsCornShader{
    /// The component that holds the settings for this shader. Can be self
    type Settings: Component+Sized+Clone+std::fmt::Debug+PartialEq;
    /// Returns how many instances a specific invocation will make
    fn get_instance_count(settings: &Self::Settings) -> u64;
    /// Returns how many workers to dispatch when invoking the shader with these settings
    fn get_invocation_count(settings: &Self::Settings) -> UVec3;
    /// Function which converts a settings component into a collection of settings buffers
    fn get_settings_buffer(settings: &Self::Settings, render_device: &RenderDevice) -> Vec<Buffer>;
}

/// Tag component for shaders that initialize the corn
#[derive(Default, Debug, Clone, PartialEq, Eq, Component)]
pub struct CornInitShader;

/// Component which identifies an entity as a init shader invocation and holds the corresponding corn fields entity, also holds the instance count.
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct CornInitInvocation(pub Entity);
impl CornInitInvocation{
    /// System which creates init shader invocations whenever the settings component for a corn field changes
    pub fn spawn_invocations<S: AsCornInitShader>(
        settings: Query<(Entity, &S::Settings), (Changed<S::Settings>, Without<CornInitShader>)>,
        shader: Single<Entity, (With<S>, With<CornInitShader>)>,
        mut commands: Commands
    ){
        for (entity, settings) in settings.iter(){
            commands.entity(*shader).with_child((
                Self(entity),
                settings.clone()
            ));
        }
    }
    /// System which creates the instance and vertex instance buffers for a init shader invocation
    pub fn create_instance_buffers<S: AsCornInitShader>(
        query: Query<(Entity, &S::Settings), (With<CornInitInvocation>, Without<InstanceBuffer>)>,
        render_device: Res<RenderDevice>, 
        mut commands: Commands 
    ){
        for (entity, settings) in query.iter(){
            let instance_count = S::get_instance_count(settings);
            let instance_buffer = InstanceBuffer::create_buffer(
                S::get_label().into().to_string() + " Instance Buffer",
                instance_count,
                &render_device
            );
            let vib = VertexInstanceBuffer::create_buffer(
                S::get_label().into().to_string() + " Vertex Instance Buffer", 
                instance_count, 
                &render_device
            );
            commands.entity(entity).insert((instance_buffer, vib));
        }
    }
}

/// Component for Init Invocations containing the settings buffers
#[derive(Default, Debug, Clone, Component)]
pub struct CornInitBuffers(pub Vec<Buffer>);
impl CornInitBuffers{
    /// System which creates the settings buffers needed for corn init shader invocations
    pub fn create_buffers<S: AsCornInitShader>(
        query: Query<(Entity, &S::Settings), (With<CornInitInvocation>, Without<Self>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, settings) in query.iter(){
            let buffers = S::get_settings_buffer(settings, &render_device);
            commands.entity(entity).insert(Self(buffers));
        }
    }
}

/// Final component added to invocations, holds the data required to invoke during the render pass
#[derive(Debug, Component)]
pub struct CornInitBinding{
    pub bindgroup: BindGroup,
    pub dispatch_count: UVec3,
    pub finished: AtomicBool
}
impl CornInitBinding{
    /// System which creates bind groups and other invocation data when the needed buffers have been created
    pub fn create_bindings<S: AsCornInitShader>(
        query: Query<(Entity, &S::Settings, &InstanceBuffer, &CornInitBuffers), (With<CornInitInvocation>, Without<Self>)>,
        shader: Single<&ShaderPipelineResources, (With<CornInitShader>, With<S>)>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, settings, instance_buffer, settings_buffers) in query.iter(){
            let dispatch_count = S::get_invocation_count(settings);
            let finished = AtomicBool::new(false);
            let mut entries = vec![BindGroupEntry{binding: 0, resource: instance_buffer.buffer.as_entire_binding()}];
            for (i, buffer) in settings_buffers.0.iter().enumerate(){
                entries.push(BindGroupEntry { binding: (i+1) as u32, resource: buffer.as_entire_binding() });
            }
            let bindgroup = render_device.create_bind_group(
                Some((S::get_label().into().to_string() + " Bind Group").as_str()), 
                &shader.layout,
                entries.as_slice()
            );
            commands.entity(entity).insert(Self{dispatch_count, finished, bindgroup});
        }
    }
    /// System which despawns finished invocations and moves their instance and vertex instance buffers to the corresponding corn field
    pub fn cleanup_bindings(
        query: Query<(Entity, &Self), With<CornInitInvocation>>,
        mut commands: Commands
    ){
        for (entity, binding) in query.iter(){
            if !binding.finished.load(Ordering::Relaxed) {continue;}
            // Move ib and vib over to the corn field, and despawn invocation
            commands.entity(entity).queue(|mut entity: EntityWorldMut|{
                let Some((invocation, instance, vib)) = 
                    entity.take::<(CornInitInvocation, InstanceBuffer, VertexInstanceBuffer)>() else {return;};
                entity.into_world_mut().entity_mut(invocation.0).insert((instance, vib, UseStoredScanPipeline));
            }).despawn();
        }
    }
}

/// Render Graph Label for Init Operations
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, RenderLabel)]
struct CornInitStage;
/// This is the render graph node which executes init shaders
#[derive(Default, Debug, Clone)]
struct CornInitNode{
    pub ready_shaders: Vec<(ComputePipeline, Vec<Entity>)>
}
impl bevy::render::render_graph::Node for CornInitNode{
    /// Updates ready_shaders to have only the pipelines which are ready and the child entities which are invocations
    fn update(&mut self, world: &mut World) {
        let mut shader_query: _ = world.query_filtered::<(&Children, &ShaderPipelineResources), With<CornInitShader>>();
        let shaders_with_children: Vec<(CachedComputePipelineId, Vec<Entity>)> = shader_query.iter(world).filter_map(
        |(children, resources)| {
            if children.is_empty() {None}
            else {Some((resources.pipeline, children.iter().collect()))}
        }).collect();
        let pipeline_cache = world.resource::<PipelineCache>();
        self.ready_shaders = shaders_with_children.into_iter().filter_map(|(id, children)| {
            pipeline_cache.get_compute_pipeline(id).map(|p| (
                p.clone(), children
            ))
        }).collect();
    }
    fn run(&self, _graph: &mut RenderGraphContext, render_context: &mut RenderContext, world: &World) -> Result<(), NodeRunError>{
        if self.ready_shaders.is_empty() {return Ok(());}
        // Start compute pass
        let mut pass = render_context.command_encoder().begin_compute_pass(&ComputePassDescriptor{
            label: Some("Corn Init Pass".into()), timestamp_writes: None
        });

        for (pipeline, children) in self.ready_shaders.iter(){
            pass.set_pipeline(&pipeline);
            for invocation in world.entity(children.as_slice()).iter(){
                let Some(settings) = invocation.get::<CornInitBinding>() else {continue;};
                pass.set_bind_group(0, &settings.bindgroup, &[]);
                pass.dispatch_workgroups(settings.dispatch_count.x, settings.dispatch_count.y, settings.dispatch_count.z);
                settings.finished.store(true, Ordering::Relaxed);
            }
        }
        Ok(())
    }
}


pub trait CornInitShaderAppExt: CornShaderAppExt{
    fn register_init_shader<S: AsCornInitShader+Default>(&mut self) -> &mut Self{
        self.insert_init_shader(S::default())
    }
    fn insert_init_shader<S: AsCornInitShader>(&mut self, shader: S) -> &mut Self;
}
impl CornInitShaderAppExt for App{
    fn insert_init_shader<S: AsCornInitShader>(&mut self, shader: S) -> &mut Self {
        self.insert_shader(shader);
        // Add init shader tag component
        let mut query: _ = self.sub_app_mut(RenderApp).world_mut().query_filtered::<Entity, (With<S>, With<CornShader>)>();
        let entity = query.single(self.sub_app_mut(RenderApp).world()).unwrap();
        self.sub_app_mut(RenderApp).world_mut().entity_mut(entity).insert(CornInitShader);
        // Schedule Systems
        self.sub_app_mut(RenderApp).add_systems(Render, (
            CornInitInvocation::spawn_invocations::<S>.in_set(RenderSet::Prepare),
            (CornInitBuffers::create_buffers::<S>,CornInitInvocation::create_instance_buffers::<S>).in_set(RenderSet::PrepareResources),
            CornInitBinding::create_bindings::<S>.in_set(RenderSet::PrepareBindGroups)
        ).chain());
        // Add extract plugins
        self.add_plugins(ExtractChangedComponentPlugin::<S::Settings>::default());
        self
    }
}

pub struct CornInitShaderPlugin;
impl Plugin for CornInitShaderPlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_systems(Render, CornInitBinding::cleanup_bindings.in_set(RenderSet::Cleanup))        
        .world_mut().resource_mut::<RenderGraph>()
            .add_node(CornInitStage, CornInitNode::default());
    }
}

