#import corn_game::{
  corn_types::{PerCornData, CornSettings, Range},
  utils::random::{randValue, randNext}
}

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
    new_location += settings[i].range.length;
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
    out.offset += (vec3<f32>(randValue(gid.x+512u*id_count.x), 0.5, randNext())*2.0-1.0)*instance_settings.random_settings.x;
    // set the random scale of the corn stalk
    out.scale = randNext() * instance_settings.height_width_min.x + instance_settings.height_width_min.y;
    // set the random rotation of the corn stalk
    let theta = randNext()*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    // enable the corn stalk
    out.enabled = 1u;
    out.uuid = 1u;
    instance_data[index.z] = out;
  }
}

@compute @workgroup_size(256, 1)
fn simple_rect_init(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  //first, determine which range the shader is currently in
  var index: vec3<u32> = vec3<u32>(0u, 0u, 0u);
  var location: u32 = 0u;
  var new_location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&settings); i++){
    new_location += settings[i].range.length;
    // Store settings id, instance index, buffer index
    index += vec3<u32>(
      i, 
      settings[i].range.instance_offset + gid.x - location, 
      gid.x - location + settings[i].range.start
    ) * u32(location<=gid.x) * u32(new_location>gid.x);
    location = new_location;
  }
  //check to see if we actually are in a range
  if gid.x < location{
    // the settings struct for the range we are currently in
    let instance_settings = settings[index.x];
    //the current index of our corn stalk in the corn field
    let instance_index: u32 = index.y;
    //The total number of instances in a row for corn stalks
    let res_width: u32 = bitcast<u32>(instance_settings.origin_res_width.w);
    // The instance coords in the field
    let instance_coords: vec2<u32> = vec2<u32>(instance_index%res_width, instance_index/res_width);
    // F32 position in our corn field
    let pos: vec2<f32> = vec2<f32>(f32(instance_coords.x), f32(instance_coords.y));
    var out: PerCornData;
    // Turn our indexes in coords by multiplying by the corn stalk spacing
    let xz_offset = pos*instance_settings.step;
    // Add the field's origin position to the corn stalk position
    out.offset = instance_settings.origin_res_width.xyz + vec3<f32>(xz_offset.x, 0.0, xz_offset.y);
    // Add random offsets to the x and z position of the corn stalk
    out.offset += (vec3<f32>(randValue(gid.x+512u*id_count.x), 0.5, randNext())*2.0-1.0)*vec3<f32>(instance_settings.random_settings.x, 1.0, instance_settings.random_settings.y);
    // set the random scale of the corn stalk
    out.scale = randNext() * instance_settings.height_width_min.x + instance_settings.height_width_min.y;
    // set the random rotation of the corn stalk
    let theta = randNext()*6.2832;
    out.rotation = vec2<f32>(sin(theta), cos(theta));
    // enable the corn stalk
    out.enabled = 1u;
    out.uuid = 2u;
    instance_data[index.z] = out;
  }
}