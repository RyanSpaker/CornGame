#import corn_game::{
  corn::{PerCornData, CornSettings},
  utils::{randValue, randNext}
}

@group(0) @binding(0)
var<storage, read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<uniform> settings: CornSettings;

@group(0) @binding(2) var path_texture: texture_2d<f32>;
@group(0) @binding(3) var path_texture_sampler: sampler;

// A hexagonal corn field created by interpreting the indices as spots on a checker pattern with one axis squished.
@compute @workgroup_size(16, 16, 1)
fn image_init(@builtin(global_invocation_id) gid: vec3<u32>) {
  let width: u32 = bitcast<u32>(settings.width);
  let instance_index: u32 = gid.x+gid.y*width/2u + (width%2u)*((gid.y+1u)/2u);
  let expanded_index: vec2<u32> = vec2<u32>(gid.x*2u+gid.y%2u, gid.y);
  var out: PerCornData;
  // Get offset
  out.offset = settings.origin;
  //out.offset += vec3<f32>(f32(instance_index)*0.1, 0.0, 0.0);
  let step = vec2<f32>(f32(expanded_index.x), f32(expanded_index.y))*settings.step;
  out.offset += vec3<f32>(mix(step.x, step.y, settings.random_settings.y), 0.0, mix(step.y, step.x, settings.random_settings.y));
  let rand = (vec2<f32>(randValue(instance_index), randNext())*2.0 - 1.0)*settings.random_settings.x;
  out.offset += vec3<f32>(rand.x, 0.0, rand.y);
  // Get scale
  out.scale = randNext() * settings.height_width_min.x + settings.height_width_min.y;
  // Get rotation
  let theta = randNext()*6.2832;
  out.rotation = vec2<f32>(sin(theta), cos(theta));
  // Set uuid
  out.uuid = 2u;
  // cutout corn that is in the path
  let uv: vec2<f32> = (out.offset - settings.origin).xz * settings.uv_scale;
  let color: vec4<f32> = textureSampleLevel(path_texture, path_texture_sampler, uv, 0.0);
  out.enabled = 4u;
  if color.r < (randNext()*0.5 + 0.5) {
      out.enabled = 0u;
  }
  // Write when valid
  if expanded_index.x < width {
    instance_data[instance_index] = out;
  }
}

@compute @workgroup_size(16, 16, 1)
fn image_rect_init(@builtin(global_invocation_id) gid: vec3<u32>) {
  let width: u32 = bitcast<u32>(settings.width);
  let instance_index: u32 = gid.x+gid.y*width;
  var out: PerCornData;
  // Get offset
  out.offset = settings.origin;
  out.offset += vec3<f32>(f32(gid.x)*settings.step.x, 0.0, f32(gid.y)*settings.step.y);
  let rand = (vec2<f32>(randValue(instance_index), randNext())*2.0 - 1.0)*settings.random_settings;
  out.offset += vec3<f32>(rand.x, 0.0, rand.y);
  // Get scale
  out.scale = randNext() * settings.height_width_min.x + settings.height_width_min.y;
  // Get rotation
  let theta = randNext()*6.2832;
  out.rotation = vec2<f32>(sin(theta), cos(theta));
  // Set uuid
  out.uuid = 2u;
  // cutout corn that is in the path
  let uv: vec2<f32> = (out.offset - settings.origin).xz * settings.uv_scale;
  let color: vec4<f32> = textureSampleLevel(path_texture, path_texture_sampler, uv, 0.0);
  out.enabled = 4u;
  if color.r < (randNext()*0.5 + 0.5) {
      out.enabled = 0u;
  }
  // Write when valid
  if gid.x < width {
    instance_data[instance_index] = out;
  }
}