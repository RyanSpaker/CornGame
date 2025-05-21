use std::borrow::Cow;

use bevy::{prelude::*, render::{extract_component::ExtractComponent, render_resource::*, renderer::RenderDevice}};
use bytemuck::{Pod, Zeroable};

use crate::ecs::corn::shader::AsCornShader;
use super::shader::{AsCornInitShader, CornInitShaderAppExt};

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct SimpleInitShaderSettings{
    origin: Vec3,
    resolution_width: u32,
    height_range: f32,
    minimum_height: f32,
    step_size: Vec2,
    random_settings: Vec4
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
pub struct SimpleInitShader{
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
impl SimpleInitShader{
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
impl AsCornShader for SimpleInitShader{
    fn load_shader(assets: &AssetServer) -> Handle<Shader> {
        assets.load("shaders/corn/init/simple_init.wgsl")
    }

    fn get_bindgroup_layout() -> Vec<BindGroupLayoutEntry> {
        vec![
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
                    ty: BufferBindingType::Uniform, 
                    has_dynamic_offset: false, 
                    min_binding_size: None },
                count: None,
            }
        ]
    }

    fn get_entry_point() -> impl Into<Cow<'static, str>> {
        "simple_rect_init"
    }

    fn get_label() -> impl Into<Cow<'static, str>> {
        "Corn Simple Init Shader"
    }
}
impl AsCornInitShader for SimpleInitShader{
    type Settings = Self;

    fn get_instance_count(settings: &Self::Settings) -> u64 {
        settings.resolution.x as u64 * settings.resolution.y as u64
    }

    fn get_settings_buffer(settings: &Self::Settings, render_device: &RenderDevice) -> Vec<Buffer> {
        let settings_struct = SimpleInitShaderSettings::from(settings);
        vec![render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Simple Corn Init Settings Buffer"),
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&[settings_struct])
        })]
    }
    
    fn get_invocation_count(settings: &Self::Settings) -> UVec3 {
        let count = Self::get_instance_count(settings);
        UVec3::new(count.div_ceil(256) as u32, 1, 1)
    }
}
impl From<&SimpleInitShader> for SimpleInitShaderSettings{
    fn from(value: &SimpleInitShader) -> Self {
        Self { 
            origin: value.get_origin(),
            height_range: value.height_range.y - value.height_range.x,
            minimum_height: value.height_range.x,
            step_size: value.get_step(),
            resolution_width: value.resolution.x,
            random_settings: value.get_random_offset_range().extend(0.0).extend(0.0)
         }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
pub struct SimpleHexagonalInitShader{
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
impl SimpleHexagonalInitShader{
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
impl AsCornShader for SimpleHexagonalInitShader{
    fn load_shader(assets: &AssetServer) -> Handle<Shader> {
        assets.load("shaders/corn/init/simple_init.wgsl")
    }

    fn get_bindgroup_layout() -> Vec<BindGroupLayoutEntry> {
        vec![
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
                    ty: BufferBindingType::Uniform, 
                    has_dynamic_offset: false, 
                    min_binding_size: None },
                count: None,
            }
        ]
    }

    fn get_entry_point() -> impl Into<Cow<'static, str>> {
        "simple_init"
    }

    fn get_label() -> impl Into<Cow<'static, str>> {
        "Corn Simple Hexagonal Init Shader"
    }
}
impl AsCornInitShader for SimpleHexagonalInitShader{
    type Settings = Self;

    fn get_instance_count(settings: &Self::Settings) -> u64 {
        let (width_res, height_res) = settings.get_resolution();
        width_res*(height_res-height_res/2)+(width_res-1)*(height_res/2)
    }

    fn get_settings_buffer(settings: &Self::Settings, render_device: &RenderDevice) -> Vec<Buffer> {
        let settings_struct = SimpleInitShaderSettings::from(settings);
        vec![render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Simple Corn Init Settings Buffer"),
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&[settings_struct])
        })]
    }
    
    fn get_invocation_count(settings: &Self::Settings) -> UVec3 {
        todo!();
        let (width, height) = settings.get_resolution();
        UVec3::new(width.div_ceil(16) as u32, height.div_ceil(16) as u32, 1)
    }
}
impl From<&SimpleHexagonalInitShader> for SimpleInitShaderSettings{
    fn from(value: &SimpleHexagonalInitShader) -> Self {
        let mut output = Self {
            origin: value.get_origin(),
            height_range: value.height_range.y - value.height_range.x,
            minimum_height: value.height_range.x,
            step_size: value.get_step(),
            resolution_width: (value.get_resolution().0*2-1) as u32,
            random_settings: Vec4::new(value.get_random_offset_range(), 0.0, 0.0, 0.0)
         };
         if !output.step_size.x.is_finite() || output.step_size.x.is_nan(){
            output.origin.x = value.center.x;
            output.step_size.x = 0.0;
         }
         if !output.step_size.y.is_finite() || output.step_size.y.is_nan(){
            output.origin.y = value.center.y;
            output.step_size.y = 0.0;
         }
         return output;
    }
}

#[derive(Default, Debug, Clone)]
pub struct SimpleInitPlugin;
impl Plugin for SimpleInitPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<SimpleInitShader>()
            .register_type::<SimpleHexagonalInitShader>()
            .register_init_shader::<SimpleInitShader>()
            .register_init_shader::<SimpleHexagonalInitShader>();
    }
}
