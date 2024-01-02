#import corn_game::corn::{PerCornData, DefragRange}

@group(0) @binding(0)
var<storage, read> old_ranges: array<DefragRange>;
@group(0) @binding(1)
var<storage, read_write> defrag_buffer: array<PerCornData>;
@group(0) @binding(2)
var<storage, read> old_data: array<PerCornData>;

@compute @workgroup_size(256, 1)
fn defragment(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  var index: vec4<u32> = vec4<u32>(0u, 0u, 0u, 0u);
  var location: u32 = 0u;
  var new_location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&old_ranges); i++){
    new_location += old_ranges[i].length;
    // If we are in this range, store the data range instance offset, the range instance offset, and the new buffer field instance offset
    index += vec4<u32>(u32(true), gid.x-location, old_ranges[i].start, old_ranges[i].instance_offset + old_ranges[i].field_offset)*u32(location<=gid.x)*u32(new_location>gid.x);
    location = new_location;
  }
  if bool(index.x){
    defrag_buffer[index.y + index.w] = old_data[index.y + index.z];
  }
}