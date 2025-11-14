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
@compute @workgroup_size(256, 1, 1)
fn image_init(@builtin(global_invocation_id) gid: vec3<u32>) {
  // only run when index is in scope
  if gid.x < arrayLength(&instance_data){
    var out: PerCornData;
    // Chessboard coords
    var index: vec2<u32> = vec2<u32>(gid.x*2u%settings.width, gid.x*2u/settings.width);
    index.x += (1u-settings.width%2u)*(index.y%2u); // Shifts middle rows when width even.
    // Get offset
    out.offset = settings.origin;
    // add step, swaps x and z depending on random settings y
    let step = vec2<f32>(f32(index.x), f32(index.y))*settings.step;
    out.offset += vec3<f32>(mix(step.x, step.y, settings.random_settings.y), 0.0, mix(step.y, step.x, settings.random_settings.y));
    // random shift
    let rand = (vec2<f32>(randValue(gid.x), randNext())*2.0 - 1.0)*settings.random_settings.x;
    out.offset += vec3<f32>(rand.x, 0.0, rand.y);
    // Get scale
    out.scale = mix(settings.height_range.x, settings.height_range.y, randNext());
    // Get rotation
    let theta = randNext()*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    // Set uuid
    out.uuid = 3u;
    // cutout corn that is in the path
    let uv: vec2<f32> = (out.offset - settings.origin).xz * settings.uv_scale;
    let color: vec4<f32> = textureSampleLevel(path_texture, path_texture_sampler, uv, 0.0);
    out.enabled = 4u;
    if color.r < (randNext()*0.5 + 0.5) {
        out.enabled = 0u;
    }
    // Write
    instance_data[gid.x] = out;
  }
}

@compute @workgroup_size(16, 16, 1)
fn image_rect_init(@builtin(global_invocation_id) gid: vec3<u32>) {
  // Only run when index is in scope
  if gid.x < settings.width {
    let instance_index: u32 = gid.x+gid.y*settings.width;
    var out: PerCornData;
    // Get offset
    out.offset = settings.origin;
    out.offset += vec3<f32>(f32(gid.x)*settings.step.x, 0.0, f32(gid.y)*settings.step.y);
    let rand = (vec2<f32>(randValue(instance_index), randNext())*2.0 - 1.0)*settings.random_settings;
    out.offset += vec3<f32>(rand.x, 0.0, rand.y);
    // Get scale
    out.scale = mix(settings.height_range.x, settings.height_range.y, randNext());
    // Get rotation
    let theta = randNext()*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    // Set uuid
    out.uuid = 4u;
    // cutout corn that is in the path
    let uv: vec2<f32> = (out.offset - settings.origin).xz * settings.uv_scale;
    let color: vec4<f32> = textureSampleLevel(path_texture, path_texture_sampler, uv, 0.0);
    out.enabled = 4u;
    if color.r < (randNext()*0.5 + 0.5) {
        out.enabled = 0u;
    }
    // Write when valid
    instance_data[instance_index] = out;
  }
}
