use std::ops::Deref;
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderContext},
        Render, RenderApp, RenderSet, Extract, render_graph::{Node, RenderGraphContext, RenderGraph}, MainWorld,
    }
};
use bytemuck::{Pod, Zeroable};
use super::{RenderedCornFields, CornFieldRenderData, CornField, CornInstanceData};

/// Enum used to track progress of corn field data
#[derive(PartialEq, Eq)]
pub enum CornFieldDataState{
    /// The field has just been created and needs to be initialized by the gpu
    Unloaded, 
    /// Data is currently being processed by the gpu
    Loading, 
    /// Data has been created, and needs to be copied back to the cpu
    Reading,
    /// Data is ready to be used for rendering
    Loaded,
    /// Data exists, but the corresponding corn field has been removed.
    /// Data is marked for deletion during prepare_rendered_corn_data()
    Stale
}
/// System to copy instance data from the renderapp to a CornInstanceData component in the main app
pub fn copy_corn_data_to_main_world(
    mut world: ResMut<MainWorld>,
    render_fields: ResMut<RenderedCornFields>
){
    if !render_fields.read_back_data{return;}
    for (entity, data) in render_fields.fields.iter(){
        if let Some(corn_data) = &data.data{
            if let Some(_field) = world.get::<CornField>(*entity){
                world.get_entity_mut(*entity).unwrap().insert(CornInstanceData{data: corn_data.clone()});
            }
        }
    }
}
/// System to copy over any new or changed corn field data to the renderapp
pub fn extract_corn_fields(
    fields: Extract<Query<(Entity, Ref<CornField>)>>,
    mut render_fields: ResMut<RenderedCornFields>
){
    let mut real_entities: Vec<Entity> = vec![];
    for (entity, field) in fields.iter(){
        if field.is_changed(){
            let mut new_data = CornFieldRenderData { 
                state: CornFieldDataState::Unloaded,
                center: field.center,
                half_extents: field.half_extents,
                resolution: field.resolution,
                height_range: field.height_range,
                data: None,
                instance_buffer: None,
                instance_buffer_bind_group: None,
                cpu_readback_buffer: None
            };
            if let Some(data) = render_fields.fields.get_mut(&entity) {
                new_data.instance_buffer = data.instance_buffer.take();
            }
            render_fields.fields.insert(entity, new_data);
        }
        real_entities.push(entity);
    }
    let keys: Vec<Entity> = render_fields.fields.keys().map(|a| a.clone()).collect();
    for key in keys.iter(){
        if !real_entities.contains(key){
            render_fields.fields.get_mut(key).unwrap().state = CornFieldDataState::Stale;
        }
    }
}
/// System to manage the render app corn fields and make sure buffers that are needed are created.
/// Also disposes of stale data
pub fn prepare_rendered_corn_data(
    mut corn_fields: ResMut<RenderedCornFields>,
    render_device: Res<RenderDevice>,
    pipeline: Res<ManageCornBuffersPipeline>
){
    let read: bool = corn_fields.read_back_data;
    corn_fields.fields.retain(|entity, data| {
        match data.state{
            CornFieldDataState::Stale => {
                if let Some(buffer) = data.instance_buffer.as_mut(){
                    buffer.destroy();
                }
                false
            }
            CornFieldDataState::Unloaded => {
                data.state = CornFieldDataState::Loading;
                if data.instance_buffer.as_ref().is_some_and(|b| b.size() != data.get_byte_count()){
                    data.instance_buffer.as_mut().unwrap().destroy();
                    data.instance_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
                        label: Some(&*("Corn Field Buffer: ".to_owned() + &*(*entity).index().to_string())), 
                        size: data.get_byte_count(), 
                        usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                        mapped_at_creation: false
                    }));
                } else if data.instance_buffer.is_none(){
                    data.instance_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
                        label: Some(&*("Corn Field Buffer: ".to_owned() + &*(*entity).index().to_string())), 
                        size: data.get_byte_count(), 
                        usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_SRC, 
                        mapped_at_creation: false
                    }));
                }
                data.cpu_readback_buffer = Some(render_device.create_buffer(&BufferDescriptor { 
                    label: Some(&*("Corn Field ReadBack Buffer: ".to_owned() + &*(*entity).index().to_string())),
                    size: data.get_byte_count(), 
                    usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ, 
                    mapped_at_creation: false 
                }));
                let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Instance Data Buffer Bind Group"),
                    layout: &pipeline.buffer_bind_group,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding { 
                            buffer: data.instance_buffer.as_ref().unwrap(), 
                            offset: 0, 
                            size: None 
                        }),
                    }],
                });
                data.instance_buffer_bind_group = Some(bind_group);
                return true;
            }
            CornFieldDataState::Loading => {
                if read {
                    data.state = CornFieldDataState::Reading;
                }else{
                    data.state = CornFieldDataState::Loaded;
                }
                return true;
            }
            CornFieldDataState::Reading => {
                data.state = CornFieldDataState::Loaded;
                if let Some(buffer) = data.cpu_readback_buffer.as_mut(){
                    data.data = Some(Vec::from(bytemuck::cast_slice(buffer.slice(..).get_mapped_range().deref())));
                    println!("{:?}", data.data.clone().unwrap());
                    buffer.destroy();
                    data.cpu_readback_buffer = None;
                }
                return true;
            }
            _ => true
        }
    });
}
/// Pipeline for the initialization of corn instance data by use of a compute shader
#[derive(Resource)]
pub struct ManageCornBuffersPipeline{
    id: CachedComputePipelineId,
    buffer_bind_group: BindGroupLayout
}
impl FromWorld for ManageCornBuffersPipeline {
    fn from_world(world: &mut World) -> Self {
        let buffer_bind_group = world.resource::<RenderDevice>().create_bind_group_layout(
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
                    }
                ],
            }
        );
        let shader = world
            .resource::<AssetServer>()
            .load("shaders/corn_init.wgsl");
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Initialize Corn Pipeline".into()),
            layout: vec![buffer_bind_group.clone()],
            push_constant_ranges: vec![PushConstantRange{stages: ShaderStages::COMPUTE, range: 0..32}],
            shader,
            shader_defs: vec![],
            entry_point: "init".into(),
        });
        Self{id: pipeline, buffer_bind_group}
    }
}
/// Struct of per corn field shader settings
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C)]
pub struct FieldSettings{
    origin: Vec3,
    height: Vec2,
    step: Vec2,
    res_width: u32,
}
impl FieldSettings{
    fn as_vec(&self) -> Vec<u8>{
        let vector = vec![
            self.origin.x, self.origin.y, self.origin.z, self.height.x, self.height.y, self.step.x, self.step.y
        ];
        let mut bytes: Vec<u8> = bytemuck::cast_slice::<f32, [u8; 4]>(vector.as_slice())
            .into_iter().flat_map(|v| v.into_iter()).map(|v| *v).collect::<Vec<u8>>();
        let end_bytes: Vec<u8> = bytemuck::cast::<u32, [u8; 4]>(self.res_width).to_vec();
        bytes.extend(end_bytes);
        return bytes;
    }
}
impl From::<&CornFieldRenderData> for FieldSettings{
    fn from(value: &CornFieldRenderData) -> Self {
        Self{
            origin: (value.center - value.half_extents.extend(0.0)),
            height: value.height_range,
            step: Vec2::new(value.half_extents.x*2.0/(value.resolution.0 as f32 - 1.0), value.half_extents.y*2.0/(value.resolution.1 as f32 - 1.0)),
            res_width: value.resolution.0 as u32
        }
    }
}
/// Node of rendergraph to run compute tasks to calculate corn positions
pub struct ManageCornBuffersNode{}
impl Node for ManageCornBuffersNode{
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        //get corn fields resource
        let fields = world.get_resource::<RenderedCornFields>();
        if fields.is_none() {return Ok(());}
        let fields = fields.unwrap();
        //get pipelines
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ManageCornBuffersPipeline>();
        //find our compute pipeline
        let compute_pipeline = pipeline_cache.get_compute_pipeline(pipeline.id);
        if compute_pipeline.is_none() {return Ok(());}
        let compute_pipeline = compute_pipeline.unwrap();
        //create compute pass
        let mut compute_pass = render_context.command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {label: Some("Initialize Corn Buffers") });
        compute_pass.set_pipeline(&compute_pipeline);
        //run buffer init code on all unloaded buffers
        fields.fields.iter()
            .filter(|(_, v)| v.state==CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            compute_pass.set_bind_group(0, data.instance_buffer_bind_group.as_ref().unwrap(), &[]);
            compute_pass.set_push_constants(0, 
                FieldSettings::from(data).as_vec().as_slice());
            compute_pass.dispatch_workgroups(
                (data.resolution.0 as f32 / 16.0).ceil() as u32, 
                (data.resolution.1 as f32 / 16.0).ceil() as u32, 
                1
            );
        });
        //drop mutable access to command encoder so that we can use it to copy buffers
        drop(compute_pass);
        //escape early if we aren't reading back the buffer data
        if !fields.read_back_data {return Ok(());}
        //go through each buffer and copy it to a cpu accessible buffer
        fields.fields.iter()
            .filter(|(_, v)| v.state==CornFieldDataState::Loading)
            .for_each(|(_, data)| 
        {
            render_context.command_encoder().copy_buffer_to_buffer(
                data.instance_buffer.as_ref().unwrap(), 
                0, 
                data.cpu_readback_buffer.as_ref().unwrap(), 
                0, 
                data.get_byte_count()
            );
        });
        //Go through each copied buffer and queue up an sync readback call
        fields.fields.iter()
            .filter(|(_, v)| v.state==CornFieldDataState::Reading)
            .for_each(|(_, data)| 
        {
            data.cpu_readback_buffer.as_ref().unwrap().slice(..).map_async(MapMode::Read, |_| {});
        });
        //Done
        return Ok(());
    }
}
/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldInitPlugin;
impl Plugin for CornFieldInitPlugin {
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<RenderedCornFields>()
            .add_systems(ExtractSchedule, (extract_corn_fields, copy_corn_data_to_main_world))
            .add_systems(
                Render,
                prepare_rendered_corn_data.in_set(RenderSet::Prepare)
            )
        .world.get_resource_mut::<RenderGraph>().unwrap()
            .add_node("Corn Buffer Init", ManageCornBuffersNode{});
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<ManageCornBuffersPipeline>();
    }
}
