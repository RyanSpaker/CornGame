use std::{ops::Range, cmp::Ordering};
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderContext, RenderQueue},
        Render, RenderApp, RenderSet, Extract, 
        render_graph::{Node, RenderGraphContext, RenderGraph}
    }, math::bool
};
use bytemuck::{Pod, Zeroable};
use super::{CornFieldRenderData, CornField, RenderedCornFields};

#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType, Default)]
#[repr(C)]
pub struct ComputeRange {
    start: u32,
    length: u32,
    id: u32,
    offset: u32
}

#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType, Default)]
#[repr(C)]
pub struct ComputeSettings {
    origin: Vec3,
    height_width_min: Vec2,
    step: Vec2,
    res_width: u32
}
impl From::<&CornFieldRenderData> for ComputeSettings{
    fn from(value: &CornFieldRenderData) -> Self {
        Self { 
            origin: value.center - value.half_extents.extend(0.0),
            height_width_min: Vec2::new(value.height_range.y-value.height_range.x, value.height_range.x),
            step: Vec2::new(
                value.half_extents.x*2.0/(value.resolution.0 as f32 - 1.0), 
                value.half_extents.y*2.0/(value.resolution.1 as f32 - 1.0)
            ),
            res_width: value.resolution.0 as u32
         }
    }
}

#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType, Default)]
#[repr(C)]
pub struct ComputeSettingsVector{ pub array: [ComputeSettings; 32] }

#[derive(PartialEq, Eq)]
pub enum CornFieldDataState{
    /// The field has just been created and needs to be initialized by the gpu
    Unloaded, 
    /// Data is currently being processed by the gpu
    Loading,
    /// Data is ready to be used for rendering
    Loaded,
    /// Data exists, but the corresponding corn field has been removed.
    /// Data is queued for deletion during prepare
    Stale
}

pub trait BufferRange{
    fn consecutive(&self, other: &Self) -> bool;
    fn combine(&mut self, other: &Self) -> &mut Self;
    fn combine_if_consecutive(&mut self, other: &Self) -> &mut Self;
}
impl BufferRange for Range<u32>{
    fn consecutive(&self, other: &Self) -> bool {
        self.end == other.start || self.start == other.end
    }
    fn combine(&mut self, other: &Self) -> &mut Self{
        self.start = self.start.min(other.start);
        self.end = self.end.max(other.end);
        return self;
    }
    fn combine_if_consecutive(&mut self, other: &Self) -> &mut Self{
        if self.consecutive(other) {
            return self.combine(other);
        }
        return self;
    }
}

pub trait BufferRanges{
    fn combine(&mut self, other: &Self) -> &mut Self;
    fn get_remaining_spaces(&self) -> u32;
    fn get_ranges(&mut self, space: u32) -> Vec<Range<u32>>;
    fn insert_or_extend(&mut self, item: Range<u32>);
    fn convert_to_compute_vec(&self, id: u32) -> Vec<ComputeRange>;
    fn count(&self) -> usize;
}
impl BufferRanges for Vec<Range<u32>>{
    fn combine(&mut self, other: &Self) -> &mut Self {
        let mut endpoints = self.iter().flat_map(|r| [(r.start, 1), (r.end, -1)].into_iter()).chain(
            other.iter().flat_map(|r| [(r.start, 1), (r.end, -1)].into_iter())
        ).collect::<Vec<(u32, i32)>>();
        endpoints.sort_by(|a, b| {
            let ord = a.0.cmp(&b.0);
            if ord == Ordering::Equal {
                return b.1.cmp(&a.1);
            }
            return ord;
        });
        let ranges = endpoints
            .into_iter()
            .scan(0, |acc, val| {
                *acc += val.1;
                if *acc == 0{
                    return Some(val.0)
                }
                if *acc == 1 && val.1 == 1{
                    return Some(val.0)
                }
                return None;
            })
            .collect::<Vec<u32>>()
            .chunks_exact(2)
            .map(|chunk| Range::<u32>{start: chunk[0], end: chunk[1]})
            .collect::<Vec<Range<u32>>>();
        *self = ranges;
        return self;
    }
    fn get_remaining_spaces(&self) -> u32 {
        self.iter().fold(0, |acc, val| acc+val.end-val.start)
    }
    fn get_ranges(&mut self, space: u32) -> Vec<Range<u32>>{
        let mut remaining_instances = space;
        let mut ranges: Vec<Range<u32>> = vec![];
        while remaining_instances > 0{
            let next_block = self[0].end - self[0].start;
            if next_block <= remaining_instances{
                remaining_instances -= next_block;
                ranges.push(self.remove(0));
            }else{
                ranges.push(Range::<u32>{start: self[0].start, end: self[0].start + remaining_instances});
                self[0].start += remaining_instances;
                remaining_instances = 0;
            }
        }
        return ranges;
    }
    fn insert_or_extend(&mut self, item: Range<u32>){
        for range in self.iter_mut(){
            if range.end == item.start{range.end = item.end; return;}
            else if range.start == item.end {range.start = item.start; return;}
        }
        self.push(item);
    }
    fn convert_to_compute_vec(&self, id: u32) -> Vec<ComputeRange> {
        self.iter().scan(0, |acc, val| {
            let range = ComputeRange{
                start: val.start,
                length: val.end - val.start,
                id,
                offset: *acc
            };
            *acc += range.length;
            Some(range)
        }).collect()
    }
    fn count(&self) -> usize {
        self.iter().map(|range| range.end - range.start).sum::<u32>() as usize
    }
}

pub struct CornInitShaderData{
    pub settings_buffer: UniformBuffer<ComputeSettingsVector>,
    pub range_buffer: DynamicStorageBuffer<ComputeRange>,
    pub bind_group: Option<BindGroup>,
    pub run_init_pass: bool,
    pub range_count: usize,
    pub corn_count: usize
}
impl CornInitShaderData{
    fn reset(&mut self) {
        self.run_init_pass = false;
    }
    fn write_buffers(
        &mut self, 
        render_device: &RenderDevice, 
        render_queue: &RenderQueue, 
        settings: &ComputeSettingsVector, 
        ranges: &Vec<ComputeRange>, 
        total_corn: usize
    ){
        self.settings_buffer.set(*settings);
        self.settings_buffer.write_buffer(&render_device, &render_queue);
        self.range_count = ranges.len();
        self.range_buffer.clear();
        for range in ranges{self.range_buffer.push(*range);}
        self.range_buffer.write_buffer(render_device, render_queue);
        self.corn_count = total_corn;
    }
}
impl Default for CornInitShaderData{
    fn default() -> Self {
        Self { 
            settings_buffer: UniformBuffer::<ComputeSettingsVector>::default(), 
            range_buffer: DynamicStorageBuffer::<ComputeRange>::default(), 
            bind_group: None, 
            run_init_pass: false, 
            range_count: 0, 
            corn_count: 0 
        }
    }
}

pub fn extract_corn_fields(
    corn_fields: Extract<Query<(Entity, Ref<CornField>)>>,
    mut corn_res: ResMut<RenderedCornFields>
){
    corn_fields.iter().filter(|(_, f)| f.is_changed()).for_each(|(e, f)| {
        corn_res.add_data(e, f.as_ref());
    });
    let entities: Vec<Entity> = corn_fields.iter().map(|(e, _)| e).collect();
    corn_res.record_stale_data(&entities);
}

pub fn prepare_rendered_corn_data(
    mut corn_fields: ResMut<RenderedCornFields>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<InitCornBuffersPipeline>
){
    let corn = corn_fields.as_mut();
    //make sure any work done in the previous frame is finished and our state is reset
    corn.settings.reset();
    //Swaps the temporary and master buffer that were switched last frame if we expanded the master buffer
    corn.buffer.finish_previous_frame_expansion();
    //Free up space from stale data
    corn.remove_stale_data_and_update_state();
    //Create master buffer if need be
    corn.init_buffer(&render_device);
    //Initialize new data with ranges and an id
    corn.init_new_data();
    //Create Corn Init settings to be used in the compute pass
    if corn.settings.run_init_pass {
        let (settings_vec, ranges, total_corn) = corn.create_settings();
        corn.settings.write_buffers(
            &render_device, 
            &render_queue, 
            &settings_vec, 
            &ranges, 
            total_corn
        );
        corn.create_bind_group(&render_device, &pipeline.buffer_bind_group);
    }
    //if need be, expand the master buffer
    corn.buffer.init_expansion(&render_device);
}

#[derive(Resource)]
pub struct InitCornBuffersPipeline{
    pub id: CachedComputePipelineId,
    buffer_bind_group: BindGroupLayout
}
impl FromWorld for InitCornBuffersPipeline {
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
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Storage { read_only: true }, 
                            has_dynamic_offset: false, 
                            min_binding_size: None },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer { 
                            ty: BufferBindingType::Uniform, 
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

pub struct InitCornBuffersNode{}
impl Node for InitCornBuffersNode{
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        //get corn fields resource
        let corn_res = world.get_resource::<RenderedCornFields>();
        if corn_res.is_none() {return Ok(());}
        let corn_res = corn_res.unwrap();
        //expand the buffer
        corn_res.buffer.run_expansion(render_context);
        corn_res.run_init(render_context, world);
        return Ok(());
    }
}

pub struct CornFieldInitPlugin;
impl Plugin for CornFieldInitPlugin {
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, extract_corn_fields)
            .add_systems(
                Render,
                prepare_rendered_corn_data.in_set(RenderSet::Prepare)
            )
        .world.get_resource_mut::<RenderGraph>().unwrap()
            .add_node("Corn Buffer Init", InitCornBuffersNode{});
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<InitCornBuffersPipeline>();
    }
}
