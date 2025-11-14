use std::{borrow::Cow, mem::swap};
use bevy::{prelude::*, render::{extract_component::ExtractComponent, render_asset::RenderAssets, render_resource::*, renderer::RenderDevice, texture::GpuImage}};
use crate::ecs::corn::shader::AsCornShader;
use super::{shader::{AsCornInitShader, CornInitShaderAppExt}, CornInitShaderSettings};

#[derive(Default, Debug, Clone, PartialEq, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
pub struct ImageCarvedShader{
    /// World Space center of the corn field
    center: Vec3,
    /// Half extents of the corn field
    half_extents: Vec2,
    /// Total resolution of the corn field
    resolution: UVec2,
    /// Min and Max height scalars
    height_range: Vec2,
    /// How much the corn can shift as a percentage of the distance between the corn normally
    rand_offset_factor: f32,
    /// Image used to carve the path
    image: Handle<Image>
}
impl ImageCarvedShader{
    /// Returns new Corn Field
    pub fn new(center: Vec3, half_extents: Vec2, resolution: UVec2, height_range: Vec2, rand_offset: f32, image: Handle<Image>) -> Self{
        Self{
            center, 
            half_extents, 
            resolution,
            height_range,
            rand_offset_factor: rand_offset,
            image
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
impl From<&ImageCarvedShader> for CornInitShaderSettings{
    fn from(value: &ImageCarvedShader) -> Self {
        Self { 
            origin: value.get_origin(),
            height_range: value.height_range,
            step_size: value.get_step(),
            resolution_width: value.resolution.x,
            random_settings: value.get_random_offset_range(),
            uv_scale: (value.half_extents*2.0).recip()
         }
    }
}
impl AsCornShader for ImageCarvedShader{
    fn load_shader(assets: &AssetServer) -> Handle<Shader> {
        assets.load("shaders/corn/init/image.wgsl")
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
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture { 
                    sample_type: TextureSampleType::Float { filterable: true }, 
                    view_dimension: TextureViewDimension::D2, 
                    multisampled: false },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None
            }
        ]
    }

    fn get_entry_point() -> impl Into<Cow<'static, str>> {
        "image_rect_init"
    }

    fn get_label() -> impl Into<Cow<'static, str>> {
        "Corn Image Carved Init Shader"
    }
}
impl AsCornInitShader for ImageCarvedShader{
    type Settings = Self;

    fn get_instance_count(settings: &Self::Settings) -> u64 {
        settings.resolution.x as u64 * settings.resolution.y as u64
    }

    fn get_settings_buffer(settings: &Self::Settings, render_device: &RenderDevice) -> Vec<Buffer> {
        let settings_struct = CornInitShaderSettings::from(settings);
        vec![render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Corn Image Carved Init Settings Buffer"),
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&[settings_struct])
        })]
    }
    
    fn get_invocation_count(settings: &Self::Settings) -> UVec3 {
        UVec3::new(settings.resolution.x.div_ceil(16), settings.resolution.y.div_ceil(16), 1)
    }

    fn append_texture_bindgroups<'a>(settings: &Self::Settings, image_assets: &'a RenderAssets<GpuImage>, entries: &mut Vec<BindGroupEntry<'a>>) {
        let Some(image) = image_assets.get(settings.image.id()) else {return;};
        let next_entry = entries.len();
        entries.push(BindGroupEntry { binding: next_entry as u32, resource: wgpu::BindingResource::TextureView(&image.texture_view) });
        entries.push(BindGroupEntry { binding: next_entry as u32+1, resource: wgpu::BindingResource::Sampler(&image.sampler) });
    }

    fn check_assets_loaded(settings: &Self::Settings, assets: &RenderAssets<GpuImage>) -> bool {
        assets.get(settings.image.id()).is_some()
    }
}
pub type ImageCarvedSettings = ImageCarvedShader;

#[derive(Default, Debug, Clone, PartialEq, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
pub struct ImageCarvedHexagonalShader{
    /// World Space center of the Corn Field
    pub center: Vec3,
    /// How far left and right the corn field extends.
    pub half_extents: Vec2,
    /// The minimum distance between adjacent pieces of corn
    pub dist_between: f32,
    /// The minimum and maximum height scalar
    pub height_range: Vec2,
    /// percentage of dist between of which corn can shift randomly
    pub rand_offset_factor: f32,
    /// Image used to carve the path
    pub image: Handle<Image>
}
impl ImageCarvedHexagonalShader{
    /// Creates new Corn Field
    pub fn new(center: Vec3, half_extents: Vec2, seperation_distance: f32, height_range: Vec2, rand_offset: f32, image: Handle<Image>) -> Self{
        Self{
            center, 
            half_extents, 
            dist_between: seperation_distance,
            height_range,
            rand_offset_factor: rand_offset,
            image
        }
    }
    /// Finds the resoltuion of expanded coords (checkerboard pattern)
    pub fn get_expanded_resolution(&self) -> UVec2{
        if self.dist_between == 0.0 {return UVec2::ZERO;}
        // Width and height of the placeable area
        let width = self.half_extents.x.max(self.half_extents.y)*2.0;
        let height = self.half_extents.x.min(self.half_extents.y)*2.0;
        // number of whole half steps along the width
        let expanded_width_res = (2.0*width/self.dist_between) as u32 + 1;
        // Rows are sqrt(3)/3 * dist/2.0 apart
        let expanded_height_res = ((6.0*height)/(self.dist_between*3f32.sqrt())) as u32+1;
        UVec2::new(expanded_width_res, expanded_height_res)
    }
    /// Returns the origin position of the corn field
    pub fn get_origin(&self) -> Vec3{
        let expanded_res = self.get_expanded_resolution();

        let mut true_width = (expanded_res.x as f32-1.0)*self.dist_between*0.5;
        let mut true_height = (expanded_res.y as f32-1.0)*self.dist_between*3f32.sqrt()/6.0;
        if self.half_extents.x < self.half_extents.y {swap(&mut true_height, &mut true_width);}
        
        return self.center - Vec3::new(true_width*0.5, 0.0, true_height*0.5);
    }
    /// Returns the step between spots on the corn field grid
    pub fn get_step(&self) -> Vec2{
        Vec2::new(
            self.dist_between*0.5, 
            self.dist_between*3f32.sqrt()/6.0
        )
    }
    /// Returns the range of distance for the random offset
    pub fn get_random_offset_range(&self) -> f32{
        return self.dist_between*self.rand_offset_factor;
    }
}
impl From<&ImageCarvedHexagonalShader> for CornInitShaderSettings{
    fn from(value: &ImageCarvedHexagonalShader) -> Self {
        let mut output = Self {
            origin: value.get_origin(),
            height_range: value.height_range,
            step_size: value.get_step(),
            resolution_width: value.get_expanded_resolution().x,
            random_settings: Vec2::new(value.get_random_offset_range(), if value.half_extents.x >= value.half_extents.y {0.0} else {1.0}),
            uv_scale: (value.half_extents*2.0).recip()
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
impl AsCornShader for ImageCarvedHexagonalShader{
    fn load_shader(assets: &AssetServer) -> Handle<Shader> {
        assets.load("shaders/corn/init/image.wgsl")
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
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture { 
                    sample_type: TextureSampleType::Float { filterable: true }, 
                    view_dimension: TextureViewDimension::D2, 
                    multisampled: false },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None
            }
        ]
    }

    fn get_entry_point() -> impl Into<Cow<'static, str>> {
        "image_init"
    }

    fn get_label() -> impl Into<Cow<'static, str>> {
        "Corn Image Carved Hexagonal Init Shader"
    }
}
impl AsCornInitShader for ImageCarvedHexagonalShader{
    type Settings = Self;

    fn get_instance_count(settings: &Self::Settings) -> u64 {
        let expanded_res = settings.get_expanded_resolution();
        if expanded_res.x%2 ==0 {
            expanded_res.x as u64*expanded_res.y as u64/2
        } else {
            expanded_res.x as u64*expanded_res.y as u64/2 + expanded_res.y.div_ceil(2) as u64
        }
    }

    fn get_settings_buffer(settings: &Self::Settings, render_device: &RenderDevice) -> Vec<Buffer> {
        let settings_struct = CornInitShaderSettings::from(settings);
        vec![render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Corn Image Carved Hexagonal Init Settings Buffer"),
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&[settings_struct])
        })]
    }
    
    fn get_invocation_count(settings: &Self::Settings) -> UVec3 {
        let instances = Self::get_instance_count(settings);
        UVec3::new(instances.div_ceil(256) as u32, 1, 1)
    }

    fn append_texture_bindgroups<'a>(settings: &Self::Settings, image_assets: &'a RenderAssets<GpuImage>, entries: &mut Vec<BindGroupEntry<'a>>) {
        let Some(image) = image_assets.get(settings.image.id()) else {return;};
        let next_entry = entries.len();
        entries.push(BindGroupEntry { binding: next_entry as u32, resource: wgpu::BindingResource::TextureView(&image.texture_view) });
        entries.push(BindGroupEntry { binding: next_entry as u32+1, resource: wgpu::BindingResource::Sampler(&image.sampler) });
    }

    fn check_assets_loaded(settings: &Self::Settings, assets: &RenderAssets<GpuImage>) -> bool {
        assets.get(settings.image.id()).is_some()
    }
}
pub type ImageCarvedHexagonalSettings = ImageCarvedHexagonalShader;

#[derive(Default, Debug, Clone)]
pub struct ImageInitPlugin;
impl Plugin for ImageInitPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<ImageCarvedShader>()
            .register_type::<ImageCarvedHexagonalShader>()
            .register_init_shader::<ImageCarvedShader>()
            .register_init_shader::<ImageCarvedHexagonalShader>();
    }
}
