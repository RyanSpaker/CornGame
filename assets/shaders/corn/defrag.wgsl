#import corn_game::corn PerCornData, Range

@group(0) @binding(0)
var<storage, read> old_ranges: array<Range>;
@group(0) @binding(1)
var<uniform> new_offsets: array<vec4<u32>, 32>;
@group(0) @binding(2)
var<storage, read_write> defrag_buffer: array<PerCornData>;
@group(0) @binding(3)
var<storage, read> old_data: array<PerCornData>;

@compute @workgroup_size(256, 1)
fn defragment(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) id_count: vec3<u32>) {
  var index: vec2<u32> = vec2<u32>(0u, 0u);
  var location: u32 = 0u;
  for (var i = 0u; i < arrayLength(&old_ranges); i++){
    let new_location = location+old_ranges[i].length;
    index += vec2<u32>(i, location)*u32(location<=gid.x)*u32(new_location>gid.x);
    location = new_location;
  }
  if gid.x < location{
    let range = old_ranges[index.x];//the range we are in
    let buffer_index: u32 = gid.x - index.y + range.start;//data location in instance buffer
    let instance_index: u32 = buffer_index - range.start + range.offset;//instance index for this corn field
    let new_offset = new_offsets[range.id].x; // per corn field offset into the new buffer
    let new_pos = new_offset + instance_index;
    defrag_buffer[new_pos] = old_data[buffer_index];
  }
}