#import bevy_pbr::mesh_functions as mesh_functions
#import bevy_pbr::skinning
#import bevy_pbr::morph
#import bevy_pbr::mesh_bindings       mesh
#import bevy_pbr::mesh_vertex_output  MeshVertexOutput

struct Vertex {
    @builtin(instance_index) instance_index: u32,
#ifdef VERTEX_POSITIONS
    @location(0) position: vec3<f32>,
#endif
#ifdef VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#ifdef VERTEX_UVS
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(3) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(4) color: vec4<f32>,
#endif
#ifdef SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
#ifdef CORN_INSTANCED
    @location(7) material_index: u32,
    @location(8) offset_scale: vec4<f32>,
    @location(9) rotation: vec2<f32>,
    @location(10) id: vec2<u32>,
#endif
#ifdef MORPH_TARGETS
    @builtin(vertex_index) index: u32,
#endif
};

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * bevy_pbr::morph::morph(vertex.index, bevy_pbr::morph::position_offset, i);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * bevy_pbr::morph::morph(vertex.index, bevy_pbr::morph::normal_offset, i);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * bevy_pbr::morph::morph(vertex.index, bevy_pbr::morph::tangent_offset, i), 0.0);
#endif
    }
    return vertex;
}
#endif

@vertex
fn vertex(vertex_no_morph: Vertex) -> MeshVertexOutput {
    var out: MeshVertexOutput;

#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif

#ifdef SKINNED
    var model = bevy_pbr::skinning::skin_model(vertex.joint_indices, vertex.joint_weights);
#else
    var model = mesh.model;
#endif

#ifdef VERTEX_NORMALS
#ifdef SKINNED
    out.world_normal = bevy_pbr::skinning::skin_normals(model, vertex.normal);
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal);
#endif
#ifdef CORN_INSTANCED
    let temp_1: f32 = dot(vertex.rotation.yx, out.world_normal.xz);
    out.world_normal.z = dot(vertex.rotation.xy, out.world_normal.xz*vec2<f32>(-1.0, 1.0));
    out.world_normal.x = temp_1;
#endif
#endif

#ifdef VERTEX_POSITIONS
#ifdef CORN_INSTANCED
    vertex.position *= vertex.offset_scale.w;
    let temp_2: f32 = dot(vertex.rotation.yx, vertex.position.xz);
    vertex.position.z = dot(vertex.rotation.xy, vertex.position.xz*vec2<f32>(-1.0, 1.0));
    vertex.position.x = temp_2;
#endif
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(vertex.position, 1.0));
#ifdef CORN_INSTANCED
    out.world_position += vec4<f32>(vertex.offset_scale.xyz, 0.0);
#endif
    out.position = mesh_functions::mesh_position_world_to_clip(out.world_position);
#endif

#ifdef VERTEX_UVS
    out.uv = vertex.uv;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, vertex.tangent);
#ifdef CORN_INSTANCED
    let temp_3: f32 = dot(vertex.rotation.yx, out.world_tangent.xz);
    out.world_tangent.z = dot(vertex.rotation.xy, out.world_tangent.xz*vec2<f32>(-1.0, 1.0));
    out.world_tangent.x = temp_3;
#endif
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

    return out;
}

@fragment
fn fragment(
    mesh: MeshVertexOutput,
) -> @location(0) vec4<f32> {
#ifdef VERTEX_COLORS
    return mesh.color;
#else
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
#endif
}