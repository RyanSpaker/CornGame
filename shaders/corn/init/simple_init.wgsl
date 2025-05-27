#import corn_game::{
  corn::{PerCornData, CornSettings},
  utils::{randValue, randNext}
}

@group(0) @binding(0)
var<storage, read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<uniform> settings: CornSettings;

@compute @workgroup_size(256, 1)
fn simple_init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  let res_width: u32 = bitcast<u32>(settings.origin_res_width.w);
  let instance_index: u32 = gid.x;
  let expanded_index: vec2<u32> = vec2<u32>(instance_index*2u%res_width, instance_index*2u/res_width);
  let pos: vec2<f32> = vec2<f32>(f32(expanded_index.x), f32(expanded_index.y));
  var out: PerCornData;
  let xz_offset = pos*settings.step;
  // Add the field's origin position to the corn stalk position
  out.offset = settings.origin_res_width.xyz + vec3<f32>(xz_offset.x, 0.0, xz_offset.y);
  // Add random offsets to the x and z position of the corn stalk
  out.offset += vec3<f32>(randValue(instance_index), 0.5, randNext())*settings.random_settings.x*2.0 - 1.0;
  // set the random scale of the corn stalk
  out.scale = randNext() * settings.height_width_min.x + settings.height_width_min.y;
  // set the random rotation of the corn stalk
  let theta = randNext()*6.2832;
  out.rotation = vec2<f32>(sin(theta), cos(theta));
  // enable the corn stalk
  out.enabled = 1u;
  out.uuid = 1u;
  instance_data[gid.x] = out;
}

@compute @workgroup_size(256, 1)
fn simple_rect_init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  let res_width: u32 = bitcast<u32>(settings.origin_res_width.w);
  let instance_index: u32 = gid.x;
  let instance_coords: vec2<u32> = vec2<u32>(instance_index%res_width, instance_index/res_width);
  let pos: vec2<f32> = vec2<f32>(f32(instance_coords.x), f32(instance_coords.y));
  var out: PerCornData;
  let xz_offset = pos*settings.step;
  // Add the field's origin position to the corn stalk position
  out.offset = settings.origin_res_width.xyz + vec3<f32>(xz_offset.x, 0.0, xz_offset.y);
  // Add random offsets to the x and z position of the corn stalk
  out.offset += vec3<f32>(randValue(instance_index), 0.5, randNext())*settings.random_settings.x*2.0 - 1.0;
  // set the random scale of the corn stalk
  out.scale = randNext() * settings.height_width_min.x + settings.height_width_min.y;
  // set the random rotation of the corn stalk
  let theta = randNext()*6.2832;
  out.rotation = vec2<f32>(sin(theta), cos(theta));
  // enable the corn stalk
  out.enabled = 1u;
  out.uuid = 1u;
  instance_data[gid.x] = out;
}