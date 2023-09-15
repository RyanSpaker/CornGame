struct PerCornData {
  offset_scale: vec4<f32>,
  rotation: vec4<f32>
}
@group(0) @binding(0)
var<storage,read_write> instance_data: array<PerCornData>;
struct Settings {
  origin: vec3<f32>,
  height: vec2<f32>,
  step: vec2<f32>,
  res_width: u32
}
var<push_constant> settings: Settings;

fn hash(value: u32) -> u32 {
    var state = value;
    state = state ^ 2747636419u;
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    return state;
}

fn randomFloat(value: u32) -> f32 {
    return f32(hash(value)) / 4294967295.0;
}

@compute @workgroup_size(16, 16, 1)
fn init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  let lid: u32 = gid.x + gid.y * (16u*id_count.x);
  let pos: vec2<f32> = vec2<f32>(f32(lid%settings.res_width), f32(lid/settings.res_width));
  var world_pos: vec4<f32> = vec4<f32>(settings.origin, 0.0);
  world_pos.x += settings.origin.x*settings.step.x;
  world_pos.y += settings.origin.y*settings.step.y;
  world_pos.w = randomFloat(lid) * (settings.height.y - settings.height.x) + settings.height.x;
  let theta = randomFloat(lid+id_count.x*id_count.y*16u*16u)*6.2832;
  let rotation: vec4<f32> = vec4<f32>(sin(theta), cos(theta), 0.0, 0.0);
  instance_data[lid] = PerCornData(world_pos, rotation);
}