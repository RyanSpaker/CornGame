#import bevy_pbr::{
    mesh_functions::mesh_position_local_to_world,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}
#import corn_game::wind::wind;

struct InstancedVertex {
    @location(8) corn_col1: vec4<f32>,
    @location(9) corn_col2: vec4<f32>,
    @location(10) corn_col3: vec4<f32>,
    @location(11) corn_col4: vec4<f32>,
}

struct Settings {
    time: f32,
    fade_in: f32,
}
@group(2) @binding(100) var<uniform> settings: Settings;

fn mesh_tangent_local_to_world(world_from_local: mat4x4<f32>, vertex_tangent: vec4<f32>) -> vec4<f32> {
    if any(vertex_tangent != vec4<f32>(0.0)) {
        return vec4<f32>(
            normalize(
                mat3x3<f32>(
                    world_from_local[0].xyz,
                    world_from_local[1].xyz,
                    world_from_local[2].xyz,
                ) * vertex_tangent.xyz
            ),
            vertex_tangent.w
        );
    } else {
        return vertex_tangent;
    }
}

@vertex
fn vertex(vertex: Vertex, instance_data: InstancedVertex) -> VertexOutput {
    var out: VertexOutput;

    var world_from_local = mat4x4<f32>(
        instance_data.corn_col1,
        instance_data.corn_col2,
        instance_data.corn_col3,
        instance_data.corn_col4,
    );
    let instance_pos_offset = instance_data.corn_col4;

    var world_from_local_rot_only = world_from_local;
    world_from_local_rot_only[3] = vec4<f32>(0.0, 0.0, 0.0, 1.0);

#ifdef VERTEX_NORMALS
    out.world_normal = (world_from_local * vec4<f32>(vertex.normal, 0.0)).xyz;
#endif // VERTEX_NORMALS

#ifdef VERTEX_POSITIONS
    // let offset_scale = world_from_local * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let rotated = world_from_local_rot_only * vec4(vertex.position, 1.0);
    let pos = wind(rotated.xyz, instance_pos_offset, settings.time);
    out.world_position = vec4<f32>(pos + instance_pos_offset.xyz, 1.0);

    // out.world_position = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);

    // TODO get random spawn_in working
    // let enabled: bool = (clamp(settings.fade_in, 0.0f, 1.0f) >= (f32(vertex.instance_index % 100u) / 100.0f));
    // out.position.z = select(10.0, out.position.z, enabled); // 10.0 is outside clip space
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    // Corn: use 0u instead of instance index
    out.instance_index = 0u;
#endif

    return out;
}