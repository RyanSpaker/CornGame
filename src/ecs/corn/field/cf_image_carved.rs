use std::{collections::hash_map::DefaultHasher, hash::{Hasher, Hash}};
use bevy::{
    app::Update, asset::{Assets, Handle}, ecs::{component::Component, system::{Query, Res}}, math::{Vec2, Vec3, Vec4}, reflect::Reflect, render::{render_resource::*, texture::Image}
};
use bytemuck::{Pod, Zeroable};
use wgpu::{util::BufferInitDescriptor, SamplerBindingType};
use crate::ecs::corn::data_pipeline::{operation_executor::{CreateInitBindgroupStructures, CreateInitBufferStructures, IntoCornPipeline, IntoOperationResources}, operation_manager::IntoBufferOperation};

use super::{
    cf_simple::SimpleHexagonalCornField, state::CornAssetState, RenderableCornField, RenderableCornFieldID
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
impl From::<(&ImageCarvedHexagonalCornField, SimpleCornFieldRange)> for SimpleCornFieldShaderSettings{
    fn from(value: (&ImageCarvedHexagonalCornField, SimpleCornFieldRange)) -> Self {
        let mut output = Self { 
            range: value.1.to_owned(),
            origin: value.0.get_origin(),
            height_range: value.0.height_range.y - value.0.height_range.x,
            minimum_height: value.0.height_range.x,
            step_size: value.0.get_step(),
            resolution_width: (value.0.get_resolution().0*2-1) as u32,
            random_settings: Vec4::new(value.0.get_random_offset_range(), 0.5/value.0.half_extents.x, 0.5/value.0.half_extents.y, 0.0)
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

/*
    Corn Fields:
*/

/// This is a Corn Field with a path carved out based on a input image. Any corn stalks on a pixel with red are kept, corn on a pixel without red are discarded
/// The corn is placed in a hexagonal pattern, making stright line patterns less common
#[derive(Clone, Component, Debug, Reflect)]
pub struct ImageCarvedHexagonalCornField{
    /// World Space center of the corn field
    center: Vec3,
    /// Half extents of the corn field
    half_extents: Vec2,
    /// minimum distance between adjacent pieces of corn
    dist_between: f32,
    /// Min and Max height scalars
    height_range: Vec2,
    /// Percent of dist between of whihc corn can shift
    rand_offset_factor: f32,
    /// The image used to carve the path
    image: Handle<Image>,
    /// A bool used to store whether the path image has been fully loaded
    image_ready: bool
}
impl ImageCarvedHexagonalCornField{
    /// Creates a new field, defaulting image_ready to false
    pub fn new(center: Vec3, half_extents: Vec2, seperation_distance: f32, height_range: Vec2, rand_offset: f32, image: Handle<Image>) -> Self{
        Self{
            center, 
            half_extents, 
            dist_between: seperation_distance,
            height_range,
            rand_offset_factor: rand_offset,
            image, image_ready: false
        }
    }
    /// Turns a simple hex corn field into an image carved hex field with the supplied image
    pub fn from_hex(field: SimpleHexagonalCornField, image: Handle<Image>) -> Self{
        let mut a: Self = field.into();
        a.image = image;
        a.image_ready = false;
        return a;
    }
    /// Returns teh resolution of the corn
    pub fn get_resolution(&self) -> (u64, u64){
        let width = self.half_extents.x.max(self.half_extents.y)*2.0;
        let height = self.half_extents.x.min(self.half_extents.y)*2.0;
        //bigger of the two width resolutions
        let width_res = (width/self.dist_between) as u64+1;
        // total height resolution of both big and small rows
        let height_res = ((2f32*height)/(self.dist_between*3f32.sqrt())) as u64+1;
        (width_res, height_res)
    }
    /// Returns the origin of the corn, bottom left corner
    pub fn get_origin(&self) -> Vec3{
        let (width_res, height_res) = self.get_resolution();
        let true_width = (width_res-1) as f32*self.dist_between;
        let true_height = (height_res-1) as f32*self.dist_between*3f32.sqrt()*0.5;
        return self.center - Vec3::new(true_width*0.5, 0.0, true_height*0.5);
    }
    /// Returns the distance between each grid position (x, y), 
    /// keep in mind that corn stalks are placed in a checkerboard pattern in this grid as a way to create the hexagonal pattern
    /// So the distance between two x adjacent stalks will actually be 2 * get_step().x
    pub fn get_step(&self) -> Vec2{
        Vec2::new(
            self.dist_between*0.5, 
            self.dist_between*3f32.sqrt()*0.5
        )
    }
    /// Returns the total distance range of the random position offset
    pub fn get_random_offset_range(&self) -> f32{
        return self.dist_between*self.rand_offset_factor;
    }
    /// A bevy system that should run once per frame. 
    /// Queries the Assets<Image> for whether or not a image is loaded, updated the image_ready bool if it is.
    pub fn update_image_state(
        mut fields: Query<&mut ImageCarvedHexagonalCornField>, 
        images: Res<Assets<Image>>
    ){
        for field in fields.iter_mut(){
            if !field.image_ready && images.get(field.image.clone()).is_some() {
                field.into_inner().image_ready = true;
            }
        }
    }
}
impl CornAssetState for ImageCarvedHexagonalCornField{
    fn assets_ready(&self) -> bool {
        return self.image_ready;
    }
}
impl IntoBufferOperation for ImageCarvedHexagonalCornField{
    fn get_instance_count(&self) -> u64 {
        let (width_res, height_res) = self.get_resolution();
        width_res*(height_res-height_res/2)+(width_res-1)*(height_res/2)
    }
}
impl IntoOperationResources for ImageCarvedHexagonalCornField{
    fn get_init_buffers(
        info: CreateInitBufferStructures<Self>
    ) -> Vec<(Buffer, Option<RenderableCornFieldID>)> {
        return info.fields.into_iter().map(
            |(field, range)| 
        {
            let data = SimpleCornFieldRange::get_ranges(range.get_continuos_ranges())
                .into_iter()
                .map(|corn_range| (field, corn_range).into())
                .collect::<Vec<SimpleCornFieldShaderSettings>>();
            let settings_buffer: Buffer = info.render_device.create_buffer_with_data(&BufferInitDescriptor{
                label: Some("Image Carved Corn Field Init Settings Buffer".into()),
                usage: BufferUsages::STORAGE,
                contents: bytemuck::cast_slice(&data[..])
            });
            return (settings_buffer, Some(field.gen_id()));
        }).collect();
    }
    fn get_init_bindgroups(
        data: CreateInitBindgroupStructures<Self>
    ) -> Vec<(BindGroup, u64)> {
        return data.fields.into_iter().filter_map(|(field, range)| {
            let total_instances: u64 = range.len();
            let id = field.gen_id();
            let Some((buffer, _)) = data.buffers.iter().filter(|(_, val)| val.as_ref().is_some_and(|val| *val == id)).next() else{
                return None;
            };
            let Some(image) = data.images.get(field.image.clone()) else {return None;};
            let init_bind_group = data.render_device.create_bind_group(
                Some("Image Carved Corn Init Bind Group".into()),
                data.layout,
                &[
                    BindGroupEntry{
                        binding: 0,
                        resource: data.operation_buffer.as_entire_binding()
                    },
                    BindGroupEntry{
                        binding: 1,
                        resource: buffer.as_entire_binding()
                    },
                    BindGroupEntry{
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&image.texture_view)
                    },
                    BindGroupEntry{
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&image.sampler)
                    },
                ],
            );
            return Some((init_bind_group, total_instances));
        }).collect();
    }
}
impl IntoCornPipeline for ImageCarvedHexagonalCornField{
    fn init_shader() -> String {
        "shaders/corn/image_init.wgsl".to_string()
    }
    fn init_entry_point() -> String {
        "simple_image_hex_init".to_string()
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
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                }
            ]
        }
    }
}
impl RenderableCornField for ImageCarvedHexagonalCornField{
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
        self.image.hash(&mut hasher);
        return hasher.finish().into();
    }
    fn add_functionality(app: &mut bevy::prelude::App) {
        app.add_systems(Update, Self::update_image_state);
    }
}
