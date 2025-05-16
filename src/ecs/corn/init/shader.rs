use std::sync::atomic::{AtomicBool, Ordering};
use bevy::{prelude::*, render::{
    extract_component::{ExtractComponent, ExtractComponentPlugin}, 
    render_graph::*, render_resource::*, 
    renderer::{RenderContext, RenderDevice}, 
    Render, RenderApp, RenderSet
}};
use crate::ecs::corn::{shader::*, CornField, CornLoaded, InstanceBuffer};

/// Component for corn fields which holds the invocation entity which will create their instance buffer
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Component)]
#[reflect(Component)] #[component(storage="SparseSet")]
pub struct WaitingOnInvocation(pub Entity);

/// Component used on children of init shader entities. Holds the corn field entity id
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Component)]
#[reflect(Component)]
pub struct InitShaderInvocation(pub Entity);

/// Component for Init Invocations containing the settings buffers
#[derive(Default, Debug, Clone, Component)]
#[component(storage="SparseSet")]
pub struct InitSettingsBuffers(pub Vec<Buffer>);

/// Final component added to invocations, holds the data required to invoke during the render pass
#[derive(Debug, Component)]
pub struct InitInvocationSettings{
    pub bindgroup: BindGroup,
    pub push_constants: Vec<u8>,
    pub dispatch_count: UVec3,
    pub finished: AtomicBool
}

/// Tag component for shaders that initialize the corn
#[derive(Default, Debug, Clone, PartialEq, Eq, Component)]
pub struct CornInitShader;

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
            else {Some((resources.pipeline, children.iter().cloned().collect()))}
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
                let Some(settings) = invocation.get::<InitInvocationSettings>() else {continue;};
                pass.set_bind_group(0, &settings.bindgroup, &[]);
                if !settings.push_constants.is_empty() {
                    pass.set_push_constants(0, settings.push_constants.as_slice());
                }
                pass.dispatch_workgroups(settings.dispatch_count.x, settings.dispatch_count.y, settings.dispatch_count.z);
                settings.finished.store(true, Ordering::Relaxed);
            }
        }
        Ok(())
    }
}

/// Functionality necessary for a init shader.
pub trait AsCornInitShader: Component+Sized+AsCornShader{
    /// The component that holds the settings for this shader. Can be self
    type Settings: Component+Sized+ExtractComponent+Clone+std::fmt::Debug;
    /// Returns how many instances a specific invocation will make
    fn get_instance_count(settings: &Self::Settings) -> u64;
    /// Returns how many workers to dispatch when invoking the shader with these settings
    fn get_invocation_count(settings: &Self::Settings) -> UVec3;
    /// Returns the push constant bytes for a invocation of the shader
    fn get_push_constants(_settings: &Self::Settings) -> Vec<u8> {vec![]}
    /// Function which converts a settings component into a collection of settings buffers
    fn get_settings_buffer(settings: &Self::Settings, render_device: &RenderDevice) -> Vec<Buffer>;
}

pub fn create_invocation_entities<S: AsCornInitShader>(
    query: Query<(Entity, &S::Settings), (Without<CornLoaded>, Without<WaitingOnInvocation>, With<CornField>)>,
    shader: Query<Entity, (With<CornInitShader>, With<S>)>,
    mut commands: Commands
){
    let shader = shader.single();
    for (entity, settings) in query.iter(){
        let invocation = commands.entity(shader).with_child((
            InitShaderInvocation(entity),
            settings.clone()
        )).id();
        commands.entity(entity).insert(WaitingOnInvocation(invocation));
    }
}

pub fn create_instance_buffers<S: AsCornInitShader>(
    query: Query<(Entity, &S::Settings), (With<InitShaderInvocation>, Without<InstanceBuffer>)>,
    render_device: Res<RenderDevice>, 
    mut commands: Commands
){
    for (entity, settings) in query.iter(){
        let instance_count = S::get_instance_count(settings);
        let instance_buffer = InstanceBuffer::create_buffer(
            S::get_label().into().to_string() + " Instance Buffer",
            instance_count,
            render_device.as_ref()
        );
        commands.entity(entity).insert(instance_buffer);
    }
}

pub fn create_settings_buffers<S: AsCornInitShader>(
    query: Query<(Entity, &S::Settings), (With<InitShaderInvocation>, Without<InitSettingsBuffers>)>,
    render_device: Res<RenderDevice>,
    mut commands: Commands
){
    for (entity, settings) in query.iter(){
        let buffers = S::get_settings_buffer(settings, render_device.as_ref());
        commands.entity(entity).insert(InitSettingsBuffers(buffers));
    }
} 

pub fn create_invocation_settings<S: AsCornInitShader>(
    query: Query<(Entity, &S::Settings, &InstanceBuffer, &InitSettingsBuffers), (With<InitShaderInvocation>, Without<InitInvocationSettings>)>,
    shader: Query<&ShaderPipelineResources, (With<CornInitShader>, With<S>)>,
    render_device: Res<RenderDevice>,
    mut commands: Commands
){
    let layout = &shader.single().layout;
    for (entity, settings, instance, buffers) in query.iter(){
        let mut entries = vec![BindGroupEntry{binding: 0, resource: instance.0.as_entire_binding()}];
        for (i, buffer) in buffers.0.iter().enumerate(){
            entries.push(BindGroupEntry { binding: (i+1) as u32, resource: buffer.as_entire_binding() });
        }
        let bindgroup = render_device.create_bind_group(
            Some((S::get_label().into().to_string() + " Bind Group").as_str()), 
            layout,
            entries.as_slice()
        );
        let push_constants = S::get_push_constants(settings);
        let dispatch_count = S::get_invocation_count(settings);
        commands.entity(entity).insert(InitInvocationSettings{
            bindgroup, push_constants, dispatch_count, finished: AtomicBool::new(false)
        });
    }
}

pub fn cleanup_invocations(
    invocations: Query<(Entity, &InitInvocationSettings), With<InitShaderInvocation>>,
    mut commands: Commands
){
    for (entity, invocation) in invocations.iter(){
        if !invocation.finished.load(Ordering::Relaxed) {continue;}
        // Move instance buffer over to the corn field, and set corn field as loaded. Finally despawn invocation
        commands.entity(entity).queue(|mut entity: EntityWorldMut|{
            let Some(InitShaderInvocation(field)) = entity.take::<InitShaderInvocation>() else {return;};
            let Some(buffer) = entity.take::<InstanceBuffer>() else {return;};
            entity.into_world_mut().entity_mut(field)
                .remove::<WaitingOnInvocation>()
                .insert((buffer, CornLoaded));
        });
        commands.entity(entity).despawn_recursive();
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
        let entity = query.single(self.sub_app_mut(RenderApp).world());
        self.sub_app_mut(RenderApp).world_mut().entity_mut(entity).insert(CornInitShader);
        // Schedule Systems
        self.sub_app_mut(RenderApp).add_systems(Render, (
            create_invocation_entities::<S>.before(RenderSet::PrepareResources).after(RenderSet::ExtractCommands), 
            (create_instance_buffers::<S>, create_settings_buffers::<S>).in_set(RenderSet::PrepareResources), 
            create_invocation_settings::<S>.in_set(RenderSet::PrepareBindGroups)
        ));
        // Add extract plugins
        self.add_plugins(ExtractComponentPlugin::<S::Settings>::default());
        self
    }
}

pub struct CornInitShaderPlugin;
impl Plugin for CornInitShaderPlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_systems(Render, cleanup_invocations.in_set(RenderSet::Cleanup))        
        .world_mut().resource_mut::<RenderGraph>()
            .add_node(CornInitStage, CornInitNode::default());
    }
}
