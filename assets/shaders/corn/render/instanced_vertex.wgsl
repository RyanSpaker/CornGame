#import bevy_pbr::{
    mesh_functions,
    skinning,
    morph::morph,
    forward_io::VertexOutput,
    mesh_bindings::mesh,
    view_transformations::position_world_to_clip,
}
#import bevy_render::maths::affine_to_square
#import corn_game::utils::{randValue}

// #import bevy_pbr::{
//     mesh_view_bindings::globals
// }

#import bevy_render::globals::Globals
@group(2) @binding(100) var<uniform> time: f32;

struct PushConstants {
    base_instance: i32,
    time: f32
}

var<push_constant> push_constants: PushConstants;

fn get_instance_index(instance_index: u32) -> u32 {
    return u32(push_constants.base_instance) + instance_index;
}

fn get_model_matrix(instance_index: u32) -> mat4x4<f32> {
    return affine_to_square(mesh[get_instance_index(instance_index)].model);
}

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
#ifdef VERTEX_UVS_B
    @location(3) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(5) color: vec4<f32>,
#endif
#ifdef SKINNED
    @location(6) joint_indices: vec4<u32>,
    @location(7) joint_weights: vec4<f32>,
#endif
#ifdef CORN_INSTANCED
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
        vertex.position += weight * morph(vertex.index, bevy_pbr::morph::position_offset, i);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * morph(vertex.index, bevy_pbr::morph::normal_offset, i);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * morph(vertex.index, bevy_pbr::morph::tangent_offset, i), 0.0);
#endif
    }
    return vertex;
}
#endif

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;

#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif

#ifdef SKINNED
    var model = skinning::skin_model(vertex.joint_indices, vertex.joint_weights);
#else
    // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
    // See https://github.com/gfx-rs/naga/issues/2416 .
    var model = get_model_matrix(0u);
#endif

#ifdef VERTEX_NORMALS
#ifdef SKINNED
    out.world_normal = skinning::skin_normals(model, vertex.normal);
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
        // See https://github.com/gfx-rs/naga/issues/2416
        get_instance_index(0u)
    );
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
    
    // WIND
    /* acerola example */
    var idHash : f32 = randValue( u32(abs(vertex.offset_scale.x * 10000 + vertex.offset_scale.y * 100 + vertex.offset_scale.z * 0.05f + 2)) );
    idHash = randValue( u32(idHash * 100000) );

    /*
    //(tex2Dlod(_WindTex, worldUV).r);
    // current wind strength adjustment, biased 0-1
    var strength : f32 = cos(push_constants.time / 5.2) / 2 + 0.5;

    // wind amount biased 0 - 1
    let speriod = 2.0;
    let tperiod = 1.0;
    let vertskew = 0.0; // I want the top to start moving before middle, TODO what is a good value

    let wind : f32 = cos((vertex.offset_scale.x + vertex.offset_scale.y)/speriod + push_constants.time / tperiod + vertex.position.y * vertskew) / 2 + .5;

    // calculate movement
    let amount: f32 = 0.05 * mix(0.2, 1.0, strength);
    let swayVariance : f32 = mix(0.5, 1.0, idHash); //randomize amount per cornstalk
    let bias: f32 = mix(-0.5, 0.0, strength); // deflect symmetrically at 0 wind strength
    var movement : f32 = (wind+bias) * swayVariance * vertex.position.y * vertex.position.y * strength;

    var new_p: vec3<f32> = vertex.position;
    new_p.x += movement;
    //new_p.z += movement;

    // calculate drop in y due to rotation
    new_p.y *= sqrt(1 - pow(movement / vertex.position.y, 2.0));
    new_p.y = mix(new_p.y, vertex.position.y, abs(vertex.position.x) / 10); // calculate position of leaves less *accurate* in order to get a stretch effect

    //flutter
    var flutter : f32 = cos(vertex.offset_scale.x + vertex.offset_scale.z + push_constants.time * 20 * (idHash+.5));
    flutter *= wind * wind; // more flutter when wind is deflecting
    flutter *= new_p.y * new_p.y; //more flutter at the top
    flutter *= vertex.position.x * vertex.position.x; //more flutter in the leaves
    flutter *= strength / 50; // use wind strength to modulate amount of flutter
    new_p.y += flutter;
    */

    //(tex2Dlod(_WindTex, worldUV).r);
    var strength : f32 = cos(push_constants.time / 5.2) / 2 + 0.5;
    var wind : f32     = cos((vertex.offset_scale.x + vertex.offset_scale.y)/2 + push_constants.time) / 2 + 0.5;

    var movement : f32 = wind + mix(-0.5, 0.1, strength); // use strength to modulate minimum deflection (at 0 strength, modulation is symmetric), total range is always 1
    movement *= vertex.position.y * vertex.position.y; // more sway at top
    movement *= mix(0.2, 1.2, strength) / 10; // use strength to modulate amount of deflection

    let swayVariance : f32 = mix(0.5, 1.0, idHash);
    movement *= swayVariance; // add some randomness per stalk

    var new_p: vec3<f32> = vertex.position;
    new_p.x -= movement; // I'm a little surprised this is negative
    new_p.z -= movement;

    // calculate drop in y due to rotation
    new_p.y *= sqrt(1 - pow(movement / vertex.position.y, 2.0));
    new_p.y = mix(new_p.y, vertex.position.y, abs(vertex.position.x) / 10); // calculate position of leaves less *accurate* in order to get a stretch effect
    
    //flutter
    var flutter : f32 = cos(vertex.offset_scale.x + vertex.offset_scale.y + push_constants.time*20 * (idHash+.5));
    flutter *= strength * wind * wind * 4;
    flutter *= new_p.y * new_p.y * vertex.position.x * vertex.position.x / 100;
    new_p.y += flutter;

    //TODO normals (this requires matrix math that I'm not looking forward to doing in wgsl)

    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(new_p, 1.0));

#ifdef CORN_INSTANCED
    out.world_position += vec4<f32>(vertex.offset_scale.xyz, 0.0);
#endif

    out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef VERTEX_UVS
    out.uv = vertex.uv;
#endif

#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        model,
        vertex.tangent,
        // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
        // See https://github.com/gfx-rs/naga/issues/2416
        get_instance_index(0u)
    );
#ifdef CORN_INSTANCED
    let temp_3: f32 = dot(vertex.rotation.yx, out.world_tangent.xz);
    out.world_tangent.z = dot(vertex.rotation.xy, out.world_tangent.xz*vec2<f32>(-1.0, 1.0));
    out.world_tangent.x = temp_3;
#endif
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
    // See https://github.com/gfx-rs/naga/issues/2416
    out.instance_index = get_instance_index(0u);
#endif

    return out;
}