use std::{collections::hash_map::DefaultHasher, hash::{Hasher, Hash}};
use bevy::{
    asset::Handle, 
    ecs::component::Component, 
    math::{Vec2, Vec3, Vec4, UVec2}, 
    reflect::Reflect, 
    render::{render_resource::*, texture::Image}
};
use bytemuck::{Pod, Zeroable};
use wgpu::util::BufferInitDescriptor;
use crate::ecs::corn::data_pipeline::{operation_executor::{IntoCornPipeline, IntoOperationResources}, operation_manager::IntoBufferOperation};
use super::{
    cf_image_carved::ImageCarvedHexagonalCornField, state::CornAssetState, RenderableCornField, RenderableCornFieldID, 
    super::data_pipeline::operation_executor::{CreateInitBindgroupStructures, CreateInitBufferStructures}
};

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
        ranges.into_iter().map(|(start, end)| {
            let cur_length = end-start;
            counter += cur_length;
            Self::new(start as u32, end as u32, (counter - cur_length) as u32)
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
impl From::<(&SimpleHexagonalCornField, SimpleCornFieldRange)> for SimpleCornFieldShaderSettings{
    fn from(value: (&SimpleHexagonalCornField, SimpleCornFieldRange)) -> Self {
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
impl From::<(&SimpleRectangularCornField, SimpleCornFieldRange)> for SimpleCornFieldShaderSettings{
    fn from(value: (&SimpleRectangularCornField, SimpleCornFieldRange)) -> Self {
        let output = Self { 
            range: value.1.to_owned(),
            origin: value.0.get_origin(),
            height_range: value.0.height_range.y - value.0.height_range.x,
            minimum_height: value.0.height_range.x,
            step_size: value.0.get_step(),
            resolution_width: value.0.resolution.x,
            random_settings: value.0.get_random_offset_range().extend(0.0).extend(0.0)
         };
         return output;
    }
}

/*
    Corn Fields:
*/

/// This is a super simple corn field implementation
/// There is no path, just a block of corn
/// The corn is placed in a hexagonal pattern, making stright line patterns less common
#[derive(Clone, Component, Debug, Reflect)]
pub struct SimpleHexagonalCornField{
    /// World Space center of the Corn Field
    center: Vec3,
    /// How far left and right the corn field extends.
    half_extents: Vec2,
    /// The minimum distance between adjacent pieces of corn
    dist_between: f32,
    /// The minimum and maximum height scalar
    height_range: Vec2,
    /// percentage of dist between of which corn can shift randomly
    rand_offset_factor: f32
}
impl SimpleHexagonalCornField{
    /// Creates new Corn Field
    pub fn new(center: Vec3, half_extents: Vec2, seperation_distance: f32, height_range: Vec2, rand_offset: f32) -> Self{
        Self{
            center, 
            half_extents, 
            dist_between: seperation_distance,
            height_range,
            rand_offset_factor: rand_offset
        }
    }
    /// Returns the resolution of the corn field in (# corn across) x (# corn down)
    pub fn get_resolution(&self) -> (u64, u64){
        let width = self.half_extents.x.max(self.half_extents.y)*2.0;
        let height = self.half_extents.x.min(self.half_extents.y)*2.0;
        //bigger of the two width resolutions
        let width_res = (width/self.dist_between) as u64+1;
        // total height resolution of both big and small rows
        let height_res = ((2f32*height)/(self.dist_between*3f32.sqrt())) as u64+1;
        (width_res, height_res)
    }
    /// Returns the origin position of the corn field
    pub fn get_origin(&self) -> Vec3{
        let (width_res, height_res) = self.get_resolution();
        let true_width = (width_res-1) as f32*self.dist_between;
        let true_height = (height_res-1) as f32*self.dist_between*3f32.sqrt()*0.5;
        return self.center - Vec3::new(true_width*0.5, 0.0, true_height*0.5);
    }
    /// Returns the step between spots on the corn field grid
    pub fn get_step(&self) -> Vec2{
        Vec2::new(
            self.dist_between*0.5, 
            self.dist_between*3f32.sqrt()*0.5
        )
    }
    /// Returns the range of distance for the random offset
    pub fn get_random_offset_range(&self) -> f32{
        return self.dist_between*self.rand_offset_factor;
    }
}
impl CornAssetState for SimpleHexagonalCornField{}
impl IntoBufferOperation for SimpleHexagonalCornField{
    fn get_instance_count(&self) -> u64 {
        let (width_res, height_res) = self.get_resolution();
        width_res*(height_res-height_res/2)+(width_res-1)*(height_res/2)
    }
}
impl IntoOperationResources for SimpleHexagonalCornField{
    fn get_init_buffers(
        data: CreateInitBufferStructures<Self>
    ) -> Vec<(Buffer, Option<RenderableCornFieldID>)> {
        let settings: Vec<SimpleCornFieldShaderSettings> = data.fields.into_iter()
            .flat_map(|(field, range)| 
        {
            SimpleCornFieldRange::get_ranges(range.get_continuos_ranges())
                .into_iter()
                .map(|corn_range| (field, corn_range))
                .collect::<Vec<(&SimpleHexagonalCornField, SimpleCornFieldRange)>>()
        }).collect::<Vec<(&SimpleHexagonalCornField, SimpleCornFieldRange)>>().into_iter().map(|a| a.into()).collect();
        let settings_buffer: Buffer = data.render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Simple Corn Field Init Settings Buffer".into()),
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&settings[..])
        });
        return vec![(settings_buffer, None)];
    }
    fn get_init_bindgroups(
        data: CreateInitBindgroupStructures<Self>
    ) -> Vec<(BindGroup, u64)> {
        let total_instances: u64 = data.fields.iter().map(|(_, range)| range.len()).sum();
        let init_bind_group = data.render_device.create_bind_group(
            Some("Simple Corn Init Bind Group".into()),
            data.layout,
            &[
                BindGroupEntry{
                    binding: 0,
                    resource: data.operation_buffer.as_entire_binding()
                },
                BindGroupEntry{
                    binding: 1,
                    resource: data.buffers[0].0.as_entire_binding()
                }
            ],
        );
        return vec![(init_bind_group, total_instances)];
    }

}
impl IntoCornPipeline for SimpleHexagonalCornField{
    fn init_shader() -> String {
        "shaders/corn/simple_init.wgsl".to_string()
    }
    fn init_entry_point() -> String {
        "simple_init".to_string()
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
}
impl RenderableCornField for SimpleHexagonalCornField{
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
}
impl Into<ImageCarvedHexagonalCornField> for SimpleHexagonalCornField{
    fn into(self) -> ImageCarvedHexagonalCornField {
        ImageCarvedHexagonalCornField::new(
            self.center, self.half_extents, 
            self.dist_between, self.height_range, 
            self.rand_offset_factor, Handle::<Image>::default()
        )
    }
}

/// This is a super simple corn field implementation
/// There is no path, just a block of corn
/// The corn is placed in a rectangular pattern, making the total number of corn pieces easily determinable
#[derive(Clone, Component, Debug, Reflect)]
pub struct SimpleRectangularCornField{
    /// World Space center of the corn field
    center: Vec3,
    /// Half extents of the corn field
    half_extents: Vec2,
    /// Total resolution of the corn field
    resolution: UVec2,
    /// Min and Max height scalars
    height_range: Vec2,
    /// How much the corn can shift as a percentage of the distance between the corn normally
    rand_offset_factor: f32
}
impl SimpleRectangularCornField{
    /// Returns new Corn Field
    pub fn new(center: Vec3, half_extents: Vec2, resolution: UVec2, height_range: Vec2, rand_offset: f32) -> Self{
        assert!(resolution != UVec2::ZERO, "Tried to create empty corn field!");
        Self{
            center, 
            half_extents, 
            resolution,
            height_range,
            rand_offset_factor: rand_offset
        }
    }
    /// Returns the origin of the corn field, bottom left corner
    pub fn get_origin(&self) -> Vec3{
        return Vec3::new(
            if self.resolution.x > 1 {self.center.x - self.half_extents.x} else {self.center.x},
            self.center.y,
            if self.resolution.y > 1 {self.center.z - self.half_extents.y} else {self.center.z},
        );
    }
    /// Returns the step vector between corn field elements
    pub fn get_step(&self) -> Vec2{
        let size = self.half_extents*2.0;
        Vec2::new(
            if self.resolution.x > 1 {size.x / (self.resolution.x as f32 - 1.0)} else {0.0},
            if self.resolution.y > 1 {size.y / (self.resolution.y as f32 - 1.0)} else {0.0}
        )
    }
    /// Returns the random offset range for corn stalks
    pub fn get_random_offset_range(&self) -> Vec2{
        return self.get_step()*self.rand_offset_factor;
    }
}
impl CornAssetState for SimpleRectangularCornField{}
impl IntoBufferOperation for SimpleRectangularCornField{
    fn get_instance_count(&self) -> u64 {
        return (self.resolution.x*self.resolution.y) as u64;
    }
}
impl IntoOperationResources for SimpleRectangularCornField{
    fn get_init_buffers(
        data: CreateInitBufferStructures<Self>
    ) -> Vec<(Buffer, Option<RenderableCornFieldID>)> {
        let settings: Vec<SimpleCornFieldShaderSettings> = data.fields.into_iter()
            .flat_map(|(field, range)| 
        {
            SimpleCornFieldRange::get_ranges(range.get_continuos_ranges())
                .into_iter()
                .map(|corn_range| (field, corn_range))
                .collect::<Vec<(&SimpleRectangularCornField, SimpleCornFieldRange)>>()
        }).collect::<Vec<(&SimpleRectangularCornField, SimpleCornFieldRange)>>().into_iter().map(|a| a.into()).collect();
        let settings_buffer: Buffer = data.render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Simple Corn Field Init Settings Buffer".into()),
            usage: BufferUsages::STORAGE,
            contents: bytemuck::cast_slice(&settings[..])
        });
        return vec![(settings_buffer, None)];
    }
    fn get_init_bindgroups(
        data: CreateInitBindgroupStructures<Self>
    ) -> Vec<(BindGroup, u64)> {
        let total_instances: u64 = data.fields.iter().map(|(_, range)| range.len()).sum();
        let init_bind_group = data.render_device.create_bind_group(
            Some("Simple Corn Init Bind Group".into()),
            data.layout,
            &[
                BindGroupEntry{
                    binding: 0,
                    resource: data.operation_buffer.as_entire_binding()
                },
                BindGroupEntry{
                    binding: 1,
                    resource: data.buffers[0].0.as_entire_binding()
                }
            ],
        );
        return vec![(init_bind_group, total_instances)];
    }
}
impl IntoCornPipeline for SimpleRectangularCornField{
    fn init_shader() -> String {
        "shaders/corn/simple_init.wgsl".to_string()
    }
    fn init_entry_point() -> String {
        "simple_rect_init".to_string()
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
}
impl RenderableCornField for SimpleRectangularCornField{
    fn gen_id(&self) -> RenderableCornFieldID {
        let mut hasher = DefaultHasher::new();
        self.center.x.to_bits().hash(&mut hasher);
        self.center.y.to_bits().hash(&mut hasher);
        self.center.z.to_bits().hash(&mut hasher);
        self.half_extents.x.to_bits().hash(&mut hasher);
        self.half_extents.y.to_bits().hash(&mut hasher);
        self.resolution.x.hash(&mut hasher);
        self.resolution.y.hash(&mut hasher);
        self.height_range.x.to_bits().hash(&mut hasher);
        self.height_range.y.to_bits().hash(&mut hasher);
        self.rand_offset_factor.to_bits().hash(&mut hasher);
        return hasher.finish().into();
    }
}

