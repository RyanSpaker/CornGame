use std::{borrow::Cow, marker::PhantomData};
use bevy::{prelude::*, render::{render_resource::*, renderer::RenderDevice, RenderApp}};
use wgpu::{BindGroupLayoutEntry, PushConstantRange};

/// Tag component attached to entities that represent a shader
#[derive(Default, Debug, Clone, PartialEq, Eq, Component)]
pub struct CornShader;

/// Component that holds the shader pipeline resources.
#[derive(Debug, Clone, PartialEq, Component)]
pub struct ShaderPipelineResources{
    pub shader: Handle<Shader>,
    pub pipeline: CachedComputePipelineId,
    pub layout: BindGroupLayout
}

/// Functionality necessary for a init shader.
pub trait AsCornShader where Self: Component+Sized{
    /// Loads the shader from disk and returns its shader handle
    fn load_shader(assets: &AssetServer) -> Handle<Shader>;
    /// Build the bind group layout
    fn get_bindgroup_layout() -> Vec<BindGroupLayoutEntry>;
    /// Returns the shader defs for this shader
    fn get_shader_defs() -> Vec<ShaderDefVal> {vec![]}
    /// Returns the push constant ranges for this shader
    fn get_push_constant_ranges() -> Vec<PushConstantRange> {vec![]}
    /// Entry point of the shader
    fn get_entry_point() -> impl Into<Cow<'static, str>>;
    /// Name of the shader
    fn get_label() -> impl Into<Cow<'static, str>>;
    /// Whether to zero-initialize the workgroup memory for this shader
    fn get_zero_init() -> bool {false}
    /// System run on startup which creates the shader, pipeline, and layout. 
    /// Expects an entity with component Self in the renderapp to attach resources to
    fn on_startup_systems(world: &mut World) {
        let assets = world.resource::<AssetServer>();
        let render_device = world.resource::<RenderDevice>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let label: String = Self::get_label().into().to_string();

        let shader = Self::load_shader(assets);
        let layout = render_device.create_bind_group_layout(
            Some((label.clone() + " LAYOUT").as_str()), 
            &Self::get_bindgroup_layout()
        );
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor{
            label: Some(Cow::Owned(label + " PIPELINE")),
            layout: vec![layout.clone()],
            push_constant_ranges: Self::get_push_constant_ranges(),
            shader: shader.clone(),
            shader_defs: Self::get_shader_defs(),
            entry_point: Self::get_entry_point().into(),
            zero_initialize_workgroup_memory: Self::get_zero_init()
        });

        // Add our resources to the shader entity
        let mut query = world.query_filtered::<Entity, (With<Self>, With<CornShader>)>();
        let entity = query.single(world);
        world.entity_mut(entity).insert(ShaderPipelineResources{
            shader, pipeline, layout
        });
    }
}

pub trait CornShaderAppExt{
    fn register_shader<S: AsCornShader+Default>(&mut self) -> &mut Self{self.insert_shader(S::default())}
    fn insert_shader<S: AsCornShader>(&mut self, shader: S) -> &mut Self;
}
impl CornShaderAppExt for App{
    fn insert_shader<S: AsCornShader>(&mut self, shader: S) -> &mut Self{
        self.sub_app_mut(RenderApp).world_mut().spawn((shader, CornShader));
        self.add_plugins(CornShaderPlugin::<S>(PhantomData::default()))
    }
}

#[derive(Debug, Clone)]
pub struct CornShaderPlugin<S: AsCornShader>(pub PhantomData<S>);
impl<S: AsCornShader> Plugin for CornShaderPlugin<S>{
    fn build(&self, _app: &mut App) {}
    fn finish(&self, app: &mut App) {
        S::on_startup_systems(app.sub_app_mut(RenderApp).world_mut());
    }
}
