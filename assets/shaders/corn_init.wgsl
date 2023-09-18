struct PerCornData {
  offset: vec3<f32>,
  scale: f32,
  rotation: vec2<f32>,
  uuid: u32,
  enabled: u32
}
struct Range {
  start: u32,
  length: u32,
  id: u32,
  offset: u32,
}
struct CornSettings {
  origin: vec3<f32>,
  height_width_min: vec2<f32>,
  step: vec2<f32>,
  res_width: u32
}
@group(0) @binding(0)
var<storage,read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<uniform> ranges: array<Range, 32>;
@group(0) @binding(2)
var<uniform> settings: array<CornSettings, 32>;
var<push_constant> range_count: vec4<u32>;

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

@compute @workgroup_size(256, 1)
fn init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  var index: vec2<u32> = vec2<u32>(0u, 0u);
  var location: u32 = 0u;
  for (var i = 0u; i < range_count.x; i++){
    let new_location = location+ranges[i].length;
    index += select(vec2<u32>(0u , 0u), vec2<u32>(i, location), (location<=gid.x && gid.x<new_location));
    location = new_location;
  }
  if gid.x < location{
    let range = ranges[index.x];
    let buffer_index: u32 = gid.x - index.y + range.start;
    let instance_index: u32 = buffer_index - range.start + range.offset;
    let instance_settings = settings[range.id];
    let pos: vec2<f32> = vec2<f32>(f32(instance_index%instance_settings.res_width), f32(instance_index/instance_settings.res_width));
    var out: PerCornData;
    out.offset = instance_settings.origin + vec3<f32>(pos*instance_settings.step, 0.0);
    out.scale = randomFloat(gid.x) * instance_settings.height_width_min.x + instance_settings.height_width_min.y;
    let theta = randomFloat(gid.x+256u*id_count.x)*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    out.uuid = 1u<<range.id;
    out.enabled = u32(range.id<32u);
    instance_data[buffer_index] = out;
  }
}