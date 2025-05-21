#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_functions,
    skinning,
    morph::morph,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

struct InstancedVertex{
    @location(8) corn_col1: vec4<f32>,
    @location(9) corn_col2: vec4<f32>,
    @location(10) corn_col3: vec4<f32>,
    @location(11) corn_col4: vec4<f32>,
}
// index of our mesh. used instead of instance index
var<push_constant> mesh_index: u32;

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    // Corn: Replace instance index with mesh_index
    let first_vertex = mesh[mesh_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph(vertex_index, bevy_pbr::morph::position_offset, i);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * morph(vertex_index, bevy_pbr::morph::normal_offset, i);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * morph(vertex_index, bevy_pbr::morph::tangent_offset, i), 0.0);
#endif
    }
    return vertex;
}
#endif

@vertex
fn vertex(vertex_no_morph: Vertex, instance_data: InstancedVertex) -> VertexOutput {
    var out: VertexOutput;

#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif

#ifdef CORN_INSTANCED
    var world_from_local = mat4x4<f32>(
        instance_data.corn_col1, 
        instance_data.corn_col2, 
        instance_data.corn_col3, 
        instance_data.corn_col4
    );
#else
#ifdef SKINNED
    var world_from_local = skinning::skin_model(vertex.joint_indices, vertex.joint_weights);
#else
    // Corn: Use mesh_index instead of instance index
    var world_from_local = mesh_functions::get_world_from_local(mesh_index);
#endif
#endif // CORN_INSTANCED

#ifdef VERTEX_NORMALS
#ifdef CORN_INSTANCED
    out.world_normal = (world_from_local*vec4<f32>(vertex.normal, 0.0)).xyz;
#else
#ifdef SKINNED
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        // Corn: use mesh_index instead of instance index
        mesh_index
    );
#endif // SKINNED
#endif // CORN_INSTANCED
#endif // VERTEX_NORMALS

#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        // Corn: use mesh_index isntead of instance index
        mesh_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    // Corn: use mesh_index instead of instance index
    out.instance_index = mesh_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    // Corn: use mesh_index instead of instance index
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        mesh_index, world_from_local[3]);
#endif

    return out;
}

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
#ifdef VERTEX_COLORS
    return mesh.color;
#else
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
#endif
}