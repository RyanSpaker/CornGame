#import corn_game::corn PerCornData, CornSettings, Range, randomFloat

@group(0) @binding(0)
var<storage, read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<storage, read> settings: array<CornSettings>;

@compute @workgroup_size(256, 1)
fn init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  var index: vec2<u32> = vec2<u32>(0u, 0u);
  var location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&settings); i++){
    let new_location = location+settings[i].range.length;
    index += vec2<u32>(i, location)*u32(location<=gid.x)*u32(new_location>gid.x);
    location = new_location;
  }
  if gid.x < location{
    let instance_settings = settings[index.x];//settings corresponding to our corn field
    let range = instance_settings.range;//the range we are in
    let buffer_index: u32 = gid.x - index.y + range.start;//data location in instance buffer
    let instance_index: u32 = buffer_index - range.start + range.offset;//instance index for this corn field
    let res_width: u32 = bitcast<u32>(instance_settings.origin_res_width.w);
    let pos: vec2<f32> = vec2<f32>(f32(instance_index%res_width), f32(instance_index/res_width));
    var out: PerCornData;
    let xz_offset = pos*instance_settings.step;
    out.offset = instance_settings.origin_res_width.xyz + vec3<f32>(xz_offset.x, 0.0, xz_offset.y);
    out.scale = randomFloat(gid.x) * instance_settings.height_width_min.x + instance_settings.height_width_min.y;
    let theta = randomFloat(gid.x+256u*id_count.x)*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    out.uuid = 0u;
    out.enabled = u32(!bool(range.stale_range));
    instance_data[buffer_index] = out;
  }
}