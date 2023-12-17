#import corn_game::corn PerCornData, StaleRange

@group(0) @binding(0)
var<storage, read_write> data_buffer: array<PerCornData>;
@group(0) @binding(1)
var<storage, read> stale_ranges: array<StaleRange>;

@compute @workgroup_size(256, 1)
fn flag_stale(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  var index: vec3<u32> = vec3<u32>(0u, 0u, 0u);
  var location: u32 = 0u;
  var new_location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&stale_ranges); i++){
    new_location += stale_ranges[i].length;
    // If we are in this range, store the data range instance offset, the range instance offset, and the new buffer field instance offset
    index += vec3<u32>(u32(true), gid.x-location, stale_ranges[i].start)*u32(location<=gid.x)*u32(new_location>gid.x);
    location = new_location;
  }
  if bool(index.x){
    data_buffer[index.y+index.z] = PerCornData();
  }
}