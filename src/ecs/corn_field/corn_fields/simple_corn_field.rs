use std::{collections::hash_map::DefaultHasher, hash::{Hasher, Hash}};
use bevy::{
    ecs::component::Component, 
    math::{Vec2, Vec3, Vec4}, 
    render::{render_resource::{
        BindGroupDescriptor, 
        BindGroupLayoutDescriptor, 
        BindGroupLayoutEntry, 
        BufferBindingType, 
        ShaderStages, 
        BindingType, 
        Buffer, 
        BindGroupLayout, 
        BindGroup,
        BindGroupEntry,
        BufferUsages
    }, renderer::RenderDevice}
};
use bytemuck::{Pod, Zeroable};
use wgpu::util::BufferInitDescriptor;
use crate::ecs::corn_field::{data_pipeline::storage_manager::BufferRange, RenderableCornField, RenderableCornFieldID};

#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C)]
struct SimpleCornFieldRange{
    start: u32,
    length: u32,
    instance_offset: u32,
    _padding: u32
}
impl SimpleCornFieldRange{
    pub fn new(start: u32, end: u32, instance_offset: u32) -> Self{
        Self{start, length: end-start, instance_offset, _padding: 0}
    }
    pub fn get_ranges(ranges: Vec<(u64, u64)>) -> Vec<SimpleCornFieldRange>{
        let mut counter = 0;
        ranges.iter().map(|(start, end)| {
            counter += end-start;
            Self::new(*start as u32, *end as u32, (counter - *end + *start) as u32)
        }).collect()
    }
}
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct SimpleCornFieldShaderSettings{
    range: SimpleCornFieldRange,
    origin: Vec3,
    resolution_width: u32,
    height_range: f32,
    minimum_height: f32,
    step_size: Vec2,
    random_settings: Vec4
}
impl From::<(&SimpleCornField, SimpleCornFieldRange)> for SimpleCornFieldShaderSettings{
    fn from(value: (&SimpleCornField, SimpleCornFieldRange)) -> Self {
        let mut output = Self { 
            range: value.1.to_owned(),
            origin: value.0.get_origin(),
            height_range: value.0.height_range.y - value.0.height_range.x,
            minimum_height: value.0.height_range.x,
            step_size: value.0.get_step(),
            resolution_width: (value.0.get_resolution().0*2-1) as u32,
            random_settings: Vec4::new(value.0.get_random_offset_range(), 0.0, 0.0, 0.0)
         };
         if !output.step_size.x.is_finite() || output.step_size.x.is_nan(){
            output.origin.x = value.0.center.x;
            output.step_size.x = 0.0;
         }
         if !output.step_size.y.is_finite() || output.step_size.y.is_nan(){
            output.origin.y = value.0.center.y;
            output.step_size.y = 0.0;
         }
         return output;
    }
}
/// This is a super simple corn field implementation
/// There is no path, just a block of corn
/// The corn is placed in a hexagonal pattern, making stright line patterns less common
#[derive(Clone, Component, Debug)]
pub struct SimpleCornField{
    center: Vec3,
    half_extents: Vec2,
    dist_between: f32,
    height_range: Vec2,
    rand_offset_factor: f32
}
impl SimpleCornField{
    pub fn new(center: Vec3, half_extents: Vec2, seperation_distance: f32, height_range: Vec2, rand_offset: f32) -> Self{
        Self{
            center, 
            half_extents, 
            dist_between: seperation_distance,
            height_range,
            rand_offset_factor: rand_offset
        }
    }
    pub fn get_resolution(&self) -> (u64, u64){
        let width = self.half_extents.x.max(self.half_extents.y)*2.0;
        let height = self.half_extents.x.min(self.half_extents.y)*2.0;
        //bigger of the two width resolutions
        let width_res = (width/self.dist_between) as u64+1;
        // total height resolution of both big and small rows
        let height_res = ((2f32*height)/(self.dist_between*3f32.sqrt())) as u64+1;
        (width_res, height_res)
    }
    pub fn get_origin(&self) -> Vec3{
        let (width_res, height_res) = self.get_resolution();
        let true_width = (width_res-1) as f32*self.dist_between;
        let true_height = (height_res-1) as f32*self.dist_between*3f32.sqrt()*0.5;
        return self.center - Vec3::new(true_width*0.5, 0.0, true_height*0.5);
    }
    pub fn get_step(&self) -> Vec2{
        Vec2::new(
            self.dist_between*0.5, 
            self.dist_between*3f32.sqrt()*0.5
        )
    }
    pub fn get_random_offset_range(&self) -> f32{
        return self.dist_between*self.rand_offset_factor;
    }
}
impl RenderableCornField for SimpleCornField{
    fn gen_id(&self) -> RenderableCornFieldID {
        let mut hasher = DefaultHasher::new();
        self.center.x.to_bits().hash(&mut hasher);
        self.center.y.to_bits().hash(&mut hasher);
        self.center.z.to_bits().hash(&mut hasher);
        self.half_extents.x.to_bits().hash(&mut hasher);
        self.half_extents.y.to_bits().hash(&mut hasher);
        self.dist_between.to_bits().hash(&mut hasher);
        self.height_range.x.to_bits().hash(&mut hasher);
        self.height_range.y.to_bits().hash(&mut hasher);
        self.rand_offset_factor.to_bits().hash(&mut hasher);
        return hasher.finish().into();
    }

    fn get_instance_count(&self) -> u64 {
        let (width_res, height_res) = self.get_resolution();
        width_res*(height_res-height_res/2)+(width_res-1)*(height_res/2)
    }

    fn init_shader() -> String {
        "shaders/corn/simple_init.wgsl".to_string()
    }

    fn init_entry_point() -> String {
        "simple_init".to_string()
    }

    fn get_init_resources(
        fields: Vec<(&Self, BufferRange, RenderableCornFieldID, String)>,
        render_device: &RenderDevice,
        layout: &BindGroupLayout,
        operation_buffer: &Buffer
    ) -> (Vec<(BindGroup, u64)>, Vec<Buffer>) {
        let total_instances: u64 = fields.iter().map(|(_, range, _, _)| range.len()).sum();
        let settings: Vec<SimpleCornFieldShaderSettings> = fields.iter()
            .flat_map(|(field, range, _, _)| 
        {
            SimpleCornFieldRange::get_ranges(range.get_continuos_ranges()).into_iter().map(|corn_range| (*field, corn_range)).collect::<Vec<(&SimpleCornField, SimpleCornFieldRange)>>()
        }).collect::<Vec<(&SimpleCornField, SimpleCornFieldRange)>>().into_iter().map(|a| a.into()).collect();
        let settings_buffer: Buffer = render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Simple Corn Field Init Settings Buffer".into()),
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&settings[..])
        });
        let init_bind_group = render_device.create_bind_group(&BindGroupDescriptor{
            label: Some("Simple Corn Init Bind Group".into()),
            layout,
            entries: &[
                BindGroupEntry{
                    binding: 0,
                    resource: operation_buffer.as_entire_binding()
                },
                BindGroupEntry{
                    binding: 1,
                    resource: settings_buffer.as_entire_binding()
                }
            ],
        });
        return (vec![(init_bind_group, total_instances)], vec![settings_buffer]);
    }

    fn init_bind_group_descriptor<'a>() -> BindGroupLayoutDescriptor<'a> {
        BindGroupLayoutDescriptor { 
            label: Some("Simple Corn Field Init Bind Group Layout".into()), 
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
                }
            ]
        }
    }

    fn assets_ready(&self) -> bool {true}

    fn needs_continuos_buffer_space() -> bool {false}

    fn init_push_constant_ranges() -> Vec<wgpu::PushConstantRange> {vec![]}

    fn init_shader_defs() -> Vec<bevy::render::render_resource::ShaderDefVal> {vec![]}
}