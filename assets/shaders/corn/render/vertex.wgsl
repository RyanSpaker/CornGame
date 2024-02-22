#import corn_game::rendering::{wind::apply_wind, vertex_io::{CornVertex, CornData, extract_data, to_standard_input}}
#if PREPASS == true
	#import bevy_pbr::{prepass_vertex::standard_vertex, prepass_io::{Vertex, VertexOutput}}
#else
	#import bevy_pbr::{standard_vertex::standard_vertex, forward_io::{Vertex, VertexOutput}}
#endif

struct PushConstants {
    base_instance: u32,
    time: f32
}
var<push_constant> push_constants: PushConstants;

// rotates a corn vertex using the rotation values. fails if VERTEX_POSITIONS is false and were not in a prepass, but that should never happen
fn rotate_scale_corn(vertex_in: CornData) -> CornData {
	var vertex = vertex_in;
	let rotation_matrix: mat3x3<f32> = mat3x3<f32>(
        vec3<f32>(vertex.rotation.y, 0.0, vertex.rotation.x),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(vertex.rotation.x*-1.0, 0.0, vertex.rotation.y)
    );
	vertex.position = rotation_matrix*vertex.position*vertex.scale;
    vertex.normal = rotation_matrix*vertex.normal;
    vertex.tangent = vec4<f32>(rotation_matrix*vertex.tangent.xyz, vertex.tangent.w);
	return vertex;
}
// This is where our Instanced Renderng alterations happen. We extract the relevant values into CornData, offset rotate scale, and then turn that into a standard material vertex before returning.
fn corn_vertex(vertex: CornVertex) -> Vertex{
	var data: CornData = extract_data(vertex);
	data = rotate_scale_corn(data);
	data = apply_wind(data, push_constants.time);
	data.position += data.offset;
    return to_standard_input(data, vertex, push_constants.base_instance);
}
// vertex program, sends vertex into our code, then the result of that into bevy's default vertex shader
@vertex
fn vertex(vertex: CornVertex) -> VertexOutput {
	return standard_vertex(corn_vertex(vertex));
}
