#import corn_game::corn_types::PerCornData

#ifdef OVERRIDE_LOD_COUNT
const LOD_COUNT = #{OVERRIDE_LOD_COUNT}u;
#else
const LOD_COUNT = 2u;
#endif
#ifdef OVERRIDE_INDIRECT_COUNT
const INDIRECT_COUNT = #{OVERRIDE_INDIRECT_COUNT}u;
#else
const INDIRECT_COUNT = 5u;
#endif

@group(0) @binding(0)
var<storage,read_write> instance_data: array<PerCornData>;
@group(0) @binding(1)
var<storage,read_write> vote_scan_buffer: array<vec2<u32>>;
var<workgroup> temp_scan_buffer: array<array<u32, 256>, LOD_COUNT>;
@group(0) @binding(2)
var<storage, read_write> count_buffer: array<array<u32, LOD_COUNT>>;

struct FrustumValues {
  mat: mat4x4<f32>,
  offset: vec4<f32>,
  wid_offset: vec4<u32>,
  lod_dists: array<f32, LOD_COUNT>
}
var<push_constant> cull_settings: FrustumValues;

fn calc_lod(position: u32) -> u32{
  var lod: u32 = LOD_COUNT;
  let pos: vec4<f32> = vec4<f32>(instance_data[position].offset.xyz, 1.0);
  let offset: vec2<f32> = pos.xz - cull_settings.offset.xz;
  let distance: f32 = dot(offset, offset);
  for (var i = LOD_COUNT; i > 0u; i--){
    if distance < cull_settings.lod_dists[i-(1u)]{
      lod = i-(1u);
    }
  }
  let projected: vec4<f32> = cull_settings.mat*pos;
  let bounds: vec3<f32> = projected.xyz / projected.w;
  var enabled: u32 = u32(
    step(bounds.x, 1.1)*
    step(-1.1, bounds.x)* 
    step(0.0, bounds.z) > 0.0 || distance < cull_settings.lod_dists[0] // always render closest corn b/c shadows
  ) * instance_data[position].enabled;
  return select(LOD_COUNT-(1u), lod, bool(enabled));
}

@compute @workgroup_size(128, 1, 1)
fn vote_scan(@builtin(global_invocation_id) gid_simple: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid_simple: vec3<u32>) {
  let wid: vec3<u32> = wid_simple + vec3<u32>(cull_settings.wid_offset.x, 0u, 0u);
  let gid: vec3<u32> = gid_simple + vec3<u32>(cull_settings.wid_offset.x*128u, 0u, 0u);
  for(var j: u32 = 0u; j < LOD_COUNT; j++){
    count_buffer[wid.x][j] = 0u;
  }
  //Vote:
  //let lod1: u32 = (instance_data[2u*gid.x].enabled + LOD_COUNT-(1u))%(LOD_COUNT);
  //let lod2: u32 = (instance_data[2u*gid.x+1u].enabled + LOD_COUNT-(1u))%(LOD_COUNT);
  let lod1: u32 = calc_lod(2u*gid.x);
  let lod2: u32 = calc_lod(2u*gid.x+1u);
  //Scan:
  temp_scan_buffer[lod1][2u*lid.x] = 1u;
  temp_scan_buffer[lod2][2u*lid.x+1u] = 1u;
  //upswing
  var offset: u32 = 1u;
  for(var i: u32 = 128u; i > 0u; i>>=1u){
    workgroupBarrier();
    if (lid.x < i){
      let ai: u32 = offset*(2u*lid.x+1u)-(1u);
      let bi: u32 = offset*(2u*lid.x+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        temp_scan_buffer[j][bi] += temp_scan_buffer[j][ai];
      }
    }
    offset *= 2u;
  }
  //record totals and delete top element
  if (lid.x==0u) {
    for(var j: u32 = 0u; j < LOD_COUNT; j++){
      count_buffer[wid.x][j] = temp_scan_buffer[j][255];
      temp_scan_buffer[j][255] = 0u;
    }
  }
  //downswing
  for(var i: u32 = 1u; i < 256u; i<<=1u){
    offset >>= 1u;
    workgroupBarrier();
    if (lid.x < i){
      let ai: u32 = offset*(2u*lid.x+1u)-(1u);
      let bi: u32 = offset*(2u*lid.x+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        let temp: u32 = temp_scan_buffer[j][ai];
        temp_scan_buffer[j][ai] = temp_scan_buffer[j][bi];
        temp_scan_buffer[j][bi] += temp;
      }
    }
  }
  workgroupBarrier();
  vote_scan_buffer[2u*gid.x] = vec2<u32>(lod1, temp_scan_buffer[lod1][2u*lid.x]);
  vote_scan_buffer[2u*gid.x+1u] = vec2<u32>(lod2, temp_scan_buffer[lod2][2u*lid.x+1u]);
}

@group(0) @binding(3)
var<storage, read_write> count_buffer2: array<array<u32, LOD_COUNT>>;

@compute @workgroup_size(128, 1, 1)
fn group_scan_1(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>) {
  for(var j: u32 = 0u; j < LOD_COUNT; j++){
    count_buffer2[wid.x][j] = 0u;
  }
  
  for(var j: u32 = 0u; j < LOD_COUNT; j++){
    temp_scan_buffer[j][2u*lid.x] = count_buffer[2u*gid.x][j];
    temp_scan_buffer[j][2u*lid.x+1u] = count_buffer[2u*gid.x+1u][j];
  }
  //upswing
  var offset: u32 = 1u;
  for(var i: u32 = 128u; i > 0u; i>>=1u){
    workgroupBarrier();
    if (lid.x < i){
      let ai: u32 = offset*(2u*lid.x+1u)-(1u);
      let bi: u32 = offset*(2u*lid.x+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        temp_scan_buffer[j][bi] += temp_scan_buffer[j][ai];
      }
    }
    offset *= 2u;
  }
  //record totals and delete top element
  if (lid.x==0u) {
    for(var j: u32 = 0u; j < LOD_COUNT; j++){
      count_buffer2[wid.x][j] = temp_scan_buffer[j][255];
      temp_scan_buffer[j][255] = 0u;
    }
  }
  //downswing
  for(var i: u32 = 1u; i < 256u; i<<=1u){
    offset >>= 1u;
    workgroupBarrier();
    if (lid.x < i){
      let ai: u32 = offset*(2u*lid.x+1u)-(1u);
      let bi: u32 = offset*(2u*lid.x+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        let temp: u32 = temp_scan_buffer[j][ai];
        temp_scan_buffer[j][ai] = temp_scan_buffer[j][bi];
        temp_scan_buffer[j][bi] += temp;
      }
    }
  }
  workgroupBarrier();
  for(var j: u32 = 0u; j < LOD_COUNT; j++){
    count_buffer[2u*gid.x][j] = temp_scan_buffer[j][2u*lid.x];
    count_buffer[2u*gid.x+1u][j] = temp_scan_buffer[j][2u*lid.x+1u];
  }
}
//Not used struct, here for reference, the indirect buffer is filled with tightly packed instances of these
//cant set the buffer type to this because storage buffer's stride length has to be a power of 2
struct DrawIndexedIndirect {
  vertex_count: u32,
  instance_count: u32,
  first_index: u32,
  vertex_offset: i32,
  first_instance: u32
}

@group(0) @binding(4)
var<storage, read_write> indirect_buffer: array<u32, INDIRECT_COUNT>;

@compute @workgroup_size(128, 1, 1)
fn group_scan_2(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
  for(var j: u32 = 0u; j < LOD_COUNT; j++){
    temp_scan_buffer[j][2u*lid.x] = count_buffer2[2u*gid.x][j];
    temp_scan_buffer[j][2u*lid.x+1u] = count_buffer2[2u*gid.x+1u][j];
  }
  //upswing
  var offset: u32 = 1u;
  for(var i: u32 = 128u; i > 0u; i>>=1u){
    workgroupBarrier();
    if (lid.x < i){
      let ai: u32 = offset*(2u*lid.x+1u)-(1u);
      let bi: u32 = offset*(2u*lid.x+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        temp_scan_buffer[j][bi] += temp_scan_buffer[j][ai];
      }
    }
    offset *= 2u;
  }
  //record totals and delete top element
  if (lid.x==0u) {
    var sum: u32 = 0u;
    for(var j: u32 = 0u; j < LOD_COUNT; j++){
      indirect_buffer[j*5u+1u] = temp_scan_buffer[j][255];
      indirect_buffer[j*5u+4u] = sum;
      sum += temp_scan_buffer[j][255];
      temp_scan_buffer[j][255] = 0u;
    }
  }
  //downswing
  for(var i: u32 = 1u; i < 256u; i<<=1u){
    offset >>= 1u;
    workgroupBarrier();
    if (lid.x < i){
      let ai: u32 = offset*(2u*lid.x+1u)-(1u);
      let bi: u32 = offset*(2u*lid.x+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        let temp: u32 = temp_scan_buffer[j][ai];
        temp_scan_buffer[j][ai] = temp_scan_buffer[j][bi];
        temp_scan_buffer[j][bi] += temp;
      }
    }
  }
  workgroupBarrier();
  for(var j: u32 = 0u; j < LOD_COUNT; j++){
    count_buffer2[2u*gid.x][j] = temp_scan_buffer[j][2u*lid.x];
    count_buffer2[2u*gid.x+1u][j] = temp_scan_buffer[j][2u*lid.x+1u];
  }
}

@group(0) @binding(5)
var<storage,read_write> instance_index_buffer: array<PerCornData>;

@compute @workgroup_size(128, 1, 1)
fn compact(@builtin(global_invocation_id) gid_simple: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>, @builtin(workgroup_id) wid_simple: vec3<u32>) {
  let wid: vec3<u32> = wid_simple + vec3<u32>(cull_settings.wid_offset.x, 0u, 0u);
  let gid: vec3<u32> = gid_simple + vec3<u32>(cull_settings.wid_offset.x*128u, 0u, 0u);
  if 2u*gid.x < arrayLength(&instance_data){
    let lod = vote_scan_buffer[2u*gid.x].x;
    if lod+1u < LOD_COUNT {
      let offset = vote_scan_buffer[2u*gid.x].y+count_buffer[gid.x>>7u][lod] + count_buffer2[gid.x>>15u][lod] + indirect_buffer[lod*5u+4u];
      instance_index_buffer[offset] = instance_data[gid.x*2u];
      instance_index_buffer[offset].uuid = lod;
    }
  }
  if 2u*gid.x+1u < arrayLength(&instance_data){
    let lod = vote_scan_buffer[2u*gid.x+1u].x;
    if lod+1u < LOD_COUNT {
      let offset = vote_scan_buffer[2u*gid.x+1u].y+count_buffer[gid.x>>7u][lod] + count_buffer2[gid.x>>15u][lod] + indirect_buffer[lod*5u+4u];
      instance_index_buffer[offset] = instance_data[gid.x*2u+1u];
      instance_index_buffer[offset].uuid = lod;
    }
  }
}