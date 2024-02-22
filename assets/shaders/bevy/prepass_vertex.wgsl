#define_import_path bevy_pbr::prepass_vertex

#import bevy_pbr::{
    prepass_bindings,
    mesh_view_bindings::{view, previous_view_proj},
    mesh_functions,
    skinning,
    morph,
    prepass_io::{Vertex, VertexOutput},
}
#ifdef DEFERRED_PREPASS
#import bevy_pbr::rgb9e5
#endif

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let weight_count = morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph::morph(vertex.index, morph::position_offset, i);
    #ifdef VERTEX_NORMALS
        vertex.normal += weight * morph::morph(vertex.index, morph::normal_offset, i);
    #endif
    #ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * morph::morph(vertex.index, morph::tangent_offset, i), 0.0);
    #endif
    }
    return vertex;
}
#endif


fn standard_vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;

    #ifdef MORPH_TARGETS
        var vertex = morph_vertex(vertex_no_morph);
    #else
        var vertex = vertex_no_morph;
    #endif

    #ifdef SKINNED
        var model = skinning::skin_model(vertex.joint_indices, vertex.joint_weights);
    #else // SKINNED
        // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
        // See https://github.com/gfx-rs/naga/issues/2416
        var model = mesh_functions::get_model_matrix(vertex_no_morph.instance_index);
    #endif // SKINNED

    out.position = mesh_functions::mesh_position_local_to_clip(model, vec4(vertex.position, 1.0));
    #ifdef DEPTH_CLAMP_ORTHO
        out.clip_position_unclamped = out.position;
        out.position.z = min(out.position.z, 1.0);
    #endif // DEPTH_CLAMP_ORTHO

    #ifdef VERTEX_UVS
        out.uv = vertex.uv;
    #endif // VERTEX_UVS

    #ifdef VERTEX_UVS_B
        out.uv_b = vertex.uv_b;
    #endif // VERTEX_UVS_B

    #ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
        #ifdef SKINNED
            out.world_normal = skinning::skin_normals(model, vertex.normal);
        #else // SKINNED
            out.world_normal = mesh_functions::mesh_normal_local_to_world(
                vertex.normal,
                // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
                // See https://github.com/gfx-rs/naga/issues/2416
                vertex_no_morph.instance_index
            );
        #endif // SKINNED

        #ifdef VERTEX_TANGENTS
            out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
                model,
                vertex.tangent,
                // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
                // See https://github.com/gfx-rs/naga/issues/2416
                vertex_no_morph.instance_index
            );
        #endif // VERTEX_TANGENTS
    #endif // NORMAL_PREPASS_OR_DEFERRED_PREPASS

    #ifdef VERTEX_COLORS
        out.color = vertex.color;
    #endif

    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(vertex.position, 1.0));

    #ifdef MOTION_VECTOR_PREPASS
        // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
        // See https://github.com/gfx-rs/naga/issues/2416
        out.previous_world_position = mesh_functions::mesh_position_local_to_world(
            mesh_functions::get_previous_model_matrix(vertex_no_morph.instance_index),
            vec4<f32>(vertex.position, 1.0)
        );
    #endif // MOTION_VECTOR_PREPASS

    #ifdef VERTEX_OUTPUT_INSTANCE_INDEX
        // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
        // See https://github.com/gfx-rs/naga/issues/2416
        out.instance_index = vertex_no_morph.instance_index;
    #endif

    return out;
}