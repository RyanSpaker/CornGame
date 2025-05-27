#import corn_game::corn::{PerCornData, VertexPerCornData}

// Total number of lods.
#ifdef OVERRIDE_LOD_COUNT
const LOD_COUNT = #{OVERRIDE_LOD_COUNT}u;
#else
const LOD_COUNT = 1u;
#endif

#ifdef OVERRIDE_PADDING_COUNT
const PADDING_COUNT = #{OVERRIDE_PADDING_COUNT}u;
#else
const PADDING_COUNT = 3u;
#endif

const INDIRECT_COUNT = LOD_COUNT*5u;

@group(0) @binding(0)
var<storage> instance_data: array<PerCornData>;
// x holds lod level, y holds corresponding lod counter
@group(0) @binding(1)
var<storage,read_write> vote_buffer: array<vec2<u32>>;
// Buffers to hold higher order prefix scans.
@group(0) @binding(2)
var<storage,read_write> count_buffer_1: array<array<u32, LOD_COUNT>>;
@group(0) @binding(3)
var<storage,read_write> count_buffer_2: array<array<u32, LOD_COUNT>>;
// Holds indirect values for drawing the mesh lods
@group(0) @binding(4)
var<storage, read_write> indirect_buffer: array<u32, INDIRECT_COUNT>;
// Hold per corn data sent to the Vertex Shader
@group(0) @binding(5)
var<storage,read_write> instance_index_buffer: array<VertexPerCornData>;
// Local memory to store scan prepass. 512 since we need temporary space to store values during the scan
var<workgroup> scan_buffer: array<array<u32, LOD_COUNT>, 256>;


struct ConfigValues {
  /// The mesh matrix for each lod level
  field_to_world: mat4x4<f32>,
  /// Field object space to camera clip space matrix
  field_to_clip: mat4x4<f32>,
  /// Camera position in field object space
  camera_pos_field_space: vec4<f32>
}
@group(0) @binding(6)
var<uniform> config: ConfigValues;

var<push_constant> lod_cutoffs: array<f32, LOD_COUNT>;

// Calculates LOD from a index into the instance data. 
// 0 is highest, LOD_COUNT-1 is lowest, LOD_COUNT is not rendered
fn calc_lod(position: u32) -> u32{
  var lod: u32 = 0;
  let pos: vec4<f32> = vec4<f32>(instance_data[position].offset.xyz, 1.0);
  let offset: vec2<f32> = pos.xz - config.camera_pos_field_space.xz;
  let distance: f32 = dot(offset, offset);
  for (var i = 0u; i < LOD_COUNT; i++){
    if distance >= lod_cutoffs[i]{
      lod += 1u;
    }
  }
  let projected: vec4<f32> = config.field_to_clip*pos;
  let bounds: vec3<f32> = projected.xyz / projected.w;
  var enabled: u32 = u32(
    step(bounds.x, 1.1)*
    step(-1.1, bounds.x)* 
    step(0.0, bounds.z) > 0.0 || distance < lod_cutoffs[0] // always render closest corn b/c shadows
  ) * instance_data[position].enabled;
  return select(LOD_COUNT, lod, bool(enabled));
}

fn calculate_vertex_data(data: PerCornData) -> VertexPerCornData{
  // multiply mesh matrix by instance matrix
  // Rotate+Scale -> Transform -> Mesh
  let instance_matrix = mat4x4<f32>(
    vec4<f32>(data.scale*data.rotation.y, 0.0, -data.scale*data.rotation.x, 0.0), 
    vec4<f32>(0.0, data.scale, 0.0, 0.0), 
    vec4<f32>(data.scale*data.rotation.x, 0.0, data.scale*data.rotation.y, 0.0), 
    vec4<f32>(data.offset, 1.0)
  );
  return VertexPerCornData(config.field_to_world*instance_matrix);
}

fn upswing(id: u32){
  // upswing
  var offset: u32 = 1u;
  for(var i: u32 = 256u; i > 1u; i>>=1u){
    workgroupBarrier();
    if (id < i){
      let ai: u32 = offset*(id+1u)-(1u);
      let bi: u32 = offset*(id+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        scan_buffer[bi][j] += scan_buffer[ai][j];
      }
    }
    offset *= 2u;
  }
}
fn downswing(id: u32) {
  // Downswing
  var offset: u32 = 256u;
  for(var i: u32 = 2u; i < 512u; i<<=1u){
    offset >>= 1u;
    workgroupBarrier();
    if (id < i){
      let ai: u32 = offset*(id+1u)-(1u);
      let bi: u32 = offset*(id+2u)-(1u);
      for(var j: u32 = 0u; j < LOD_COUNT; j++){
        let temp: u32 = scan_buffer[ai][j];
        scan_buffer[ai][j] = scan_buffer[bi][j];
        scan_buffer[bi][j] += temp;
      }
    }
  }
}

@compute @workgroup_size(128, 1, 1)
fn vote_scan(
  // workgroup_id*workgroup_size+local_invocation_id=global_invocation_id
  @builtin(global_invocation_id) simple_gid: vec3<u32>, 
  @builtin(local_invocation_id) simple_lid: vec3<u32>, 
  @builtin(workgroup_id) wid: vec3<u32>
) {
  let lid: u32 = 2u*simple_lid.x;
  let gid: u32 = 2u*simple_gid.x;
  // Populate vote_buffer and scan_buffer with vote data
  let loda = calc_lod(gid);
  let lodb = calc_lod(gid+1u);
  vote_buffer[gid].x = loda; vote_buffer[gid+1u].x = lodb;
  scan_buffer[lid][loda] += u32(loda<LOD_COUNT); scan_buffer[lid+1u][lodb] += u32(lodb<LOD_COUNT);

  upswing(lid);
  // Record maximum in count
  if (lid < LOD_COUNT) {
    count_buffer_1[wid.x][lid] = scan_buffer[255][lid];
    scan_buffer[255][lid] = 0u;
  }
  downswing(lid);

  // place scan and lod info into the vote_buffer
  vote_buffer[gid].y = scan_buffer[lid][loda];
  vote_buffer[gid+1u].y = scan_buffer[lid+1u][lodb];
}


@compute @workgroup_size(128, 1, 1)
fn group_scan(
  // workgroup_id*workgroup_size+local_invocation_id=global_invocation_id
  @builtin(global_invocation_id) simple_gid: vec3<u32>, 
  @builtin(local_invocation_id) simple_lid: vec3<u32>, 
  @builtin(workgroup_id) wid: vec3<u32>
) {
  let lid: u32 = 2u*simple_lid.x;
  let gid: u32 = 2u*simple_gid.x;
  // Populate scan_buffer with data from count_buffer_1
  for(var j: u32 = 0; j < LOD_COUNT; j++){
    scan_buffer[lid][j] = count_buffer_1[gid][j]; 
    scan_buffer[lid+1u][j] = count_buffer_1[gid+1u][j]; 
  }

  upswing(lid);
  // Record maximum in count 2
  if (lid < LOD_COUNT) {
    count_buffer_2[wid.x][lid] = scan_buffer[255][lid];
    scan_buffer[255][lid] = 0u;
  }
  downswing(lid);

  // place scan info into the count_buffer_1
  for(var j: u32 = 0; j < LOD_COUNT; j++){
    count_buffer_1[gid][j] = scan_buffer[lid][j]; 
    count_buffer_1[gid+1u][j] = scan_buffer[lid+1u][j]; 
  }
  
}

@compute @workgroup_size(128, 1, 1)
fn group_scan2(
  // workgroup_id*workgroup_size+local_invocation_id=global_invocation_id
  @builtin(global_invocation_id) simple_gid: vec3<u32>, 
  @builtin(local_invocation_id) simple_lid: vec3<u32>
) {
  let lid: u32 = 2u*simple_lid.x;
  let gid: u32 = 2u*simple_gid.x;
  // Populate scan_buffer with data from count_buffer_1
  for(var j: u32 = 0; j < LOD_COUNT; j++){
    scan_buffer[lid][j] = count_buffer_2[gid][j]; 
    scan_buffer[lid+1u][j] = count_buffer_2[gid+1u][j]; 
  }
 
  upswing(lid);
  // Record maximum in count 2
  if (lid == 0u) {
    var sum: u32 = 0u;
    for(var j: u32 = 0u; j < LOD_COUNT; j++){
      indirect_buffer[j*5u+1u] = scan_buffer[255][j];
      indirect_buffer[j*5u+4u] = sum;
      sum += scan_buffer[255][j];
      scan_buffer[255][j] = 0u;
    }
  }
  downswing(lid);

  // place scan info into the count_buffer_1
  for(var j: u32 = 0; j < LOD_COUNT; j++){
    count_buffer_2[gid][j] = scan_buffer[lid][j]; 
    count_buffer_2[gid+1u][j] = scan_buffer[lid+1u][j]; 
  }
}

@compute @workgroup_size(128, 1, 1)
fn compact(
  // workgroup_id*workgroup_size+local_invocation_id=global_invocation_id
  @builtin(global_invocation_id) simple_gid: vec3<u32>
) {
  var gid: u32 = 2u*simple_gid.x;
  if gid < arrayLength(&instance_data){
    let lod = vote_buffer[gid].x;
    if lod < LOD_COUNT{
      let offset = vote_buffer[gid].y + 
        count_buffer_1[gid>>8u][lod] + 
        count_buffer_2[gid>>16u][lod] + 
        indirect_buffer[lod*5u+4u];
      instance_index_buffer[offset] = calculate_vertex_data(instance_data[gid]);
    }
  }
  gid += 1u;
  if gid < arrayLength(&instance_data){
    let lod = vote_buffer[gid].x;
    if lod < LOD_COUNT{
      let offset = vote_buffer[gid].y + 
        count_buffer_1[gid>>8u][lod] + 
        count_buffer_2[gid>>16u][lod] + 
        indirect_buffer[lod*5u+4u];
      instance_index_buffer[offset] = calculate_vertex_data(instance_data[gid]);
    }
  }
}