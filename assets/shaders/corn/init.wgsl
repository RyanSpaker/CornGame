#import corn_game::corn PerCornData, CornSettings, Range, randomFloat

@group(0) @binding(0)
var<storage, read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<storage, read> settings: array<CornSettings>;

@compute @workgroup_size(256, 1)
fn init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
//first, determine which range the shader is currently in
  var index: vec2<u32> = vec2<u32>(0u, 0u);
  var location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&settings); i++){
    let new_location = location+settings[i].range.length;
    index += vec2<u32>(i, location)*u32(location<=gid.x)*u32(new_location>gid.x);
    location = new_location;
  }
//check to see if we are actually within a range
  if gid.x < location{
    // the settings struct for the range we are currently in
    let instance_settings = settings[index.x];
    //the range settings
    let range = instance_settings.range;
    //our current index into the instance buffer
    let buffer_index: u32 = gid.x - index.y + range.start;
    //the current index of our corn stalk in the corn field
    let instance_index: u32 = buffer_index - range.start + range.offset;
    //The total number of expanded locations in a row for corn stalks
    let res_width: u32 = bitcast<u32>(instance_settings.origin_res_width.w);
    // the expanded index of our corn stalk. 
    // Normal indices would be a homogenous array, 
    // whereas the expanded index would be like an index into a chessboard, 
    // where only black tiles are stalks
    let expanded_index: vec2<u32> = vec2<u32>(instance_index*2u%res_width, instance_index*2u/res_width);
    // F32 position in our corn field
    let pos: vec2<f32> = vec2<f32>(f32(expanded_index.x), f32(expanded_index.y));
    var out: PerCornData;
    let xz_offset = pos*instance_settings.step;
    out.offset = instance_settings.origin_res_width.xyz + vec3<f32>(xz_offset.x, 0.0, xz_offset.y);
    out.offset += vec3<f32>(randomFloat(gid.x+512u*id_count.x), 0.0, randomFloat(gid.x+768u*id_count.x))*instance_settings.random_settings.x;
    out.scale = randomFloat(gid.x) * instance_settings.height_width_min.x + instance_settings.height_width_min.y;
    let theta = randomFloat(gid.x+256u*id_count.x)*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    out.uuid = 0u;
    out.enabled = u32(!bool(range.stale_range));
    instance_data[buffer_index] = out;
  }
}