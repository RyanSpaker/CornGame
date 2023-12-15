#import corn_game::corn PerCornData, CornSettings, Range, randomFloat

@group(0) @binding(0)
var<storage, read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<storage, read> settings: array<CornSettings>;

@compute @workgroup_size(256, 1)
fn simple_init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  //first, determine which range the shader is currently in
  var index: vec3<u32> = vec3<u32>(0u, 0u, 0u);
  var location: u32 = 0u;
  var new_location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&settings); i++){
    let new_location = new_location + settings[i].range.length;
    // Store settings id, instance index, buffer index
    index += vec3<u32>(i, settings[i].range.instance_offset + gid.x - location, gid.x - location + settings[i].range.start)*u32(location<=gid.x)*u32(new_location>gid.x);
    location = new_location;
  }
  //check to see if we actually are in a range
  if gid.x < location{
    // the settings struct for the range we are currently in
    let instance_settings = settings[index.x];
    //the current index of our corn stalk in the corn field
    let instance_index: u32 = index.y;
    //The total number of expanded locations in a row for corn stalks
    let res_width: u32 = bitcast<u32>(instance_settings.origin_res_width.w);
    // the expanded index of our corn stalk. 
    // Normal indices would be a homogenous array, 
    // whereas the expanded index would be like an index into a chessboard, 
    // where only black tiles are stalks.
    let expanded_index: vec2<u32> = vec2<u32>(instance_index*2u%res_width, instance_index*2u/res_width);
    // F32 position in our corn field
    let pos: vec2<f32> = vec2<f32>(f32(expanded_index.x), f32(expanded_index.y));
    var out: PerCornData;
    // Turn our indexes in coords by multiplying by the corn stalk spacing
    // Step will have non-equal step values even for square fields in order to stretch the checkerboard pattern into a hex pattern
    let xz_offset = pos*instance_settings.step;
    // Add the field's origin position to the corn stalk position
    out.offset = instance_settings.origin_res_width.xyz + vec3<f32>(xz_offset.x, 0.0, xz_offset.y);
    // Add random offsets to the x and z position of the corn stalk
    out.offset += vec3<f32>(randomFloat(gid.x+512u*id_count.x), 0.0, randomFloat(gid.x+768u*id_count.x))*instance_settings.random_settings.x;
    // set the random scale of the corn stalk
    out.scale = randomFloat(gid.x) * instance_settings.height_width_min.x + instance_settings.height_width_min.y;
    // set the random rotation of the corn stalk
    let theta = randomFloat(gid.x+256u*id_count.x)*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    // enable the corn stalk
    out.enabled = 1u;
    instance_data[index.z] = out;
  }
}