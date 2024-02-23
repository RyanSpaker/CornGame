#import corn_game::rendering::{wind::apply_wind_acerola, vertex_io::{CornVertex, CornData, CornVertexOutput, extract_data, to_standard_input, to_corn_output, pbr_vertex}}

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
// This is where our Instanced Renderng alterations happen. We extract the relevant values into CornData, offset rotate scale, apply_wind, and then turn that into a standard material vertex before returning.
fn corn_vertex(vertex: CornVertex) -> CornData{
	var data: CornData = extract_data(vertex);
	data = rotate_scale_corn(data);
	data = apply_wind_acerola(data, push_constants.time);
	data.position += data.offset;
    return data;
}
// vertex program, sends vertex into our code, then the result of that into bevy's default vertex shader
@vertex
fn vertex(vertex: CornVertex) -> CornVertexOutput {
	let data = corn_vertex(vertex);
	let standard_material_out = pbr_vertex(to_standard_input(data, vertex, push_constants.base_instance));
	return to_corn_output(data, vertex, standard_material_out);
}
