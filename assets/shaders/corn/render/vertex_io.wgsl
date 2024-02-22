#define_import_path corn_game::rendering::vertex_io

/*
	This File creates different functions used to convert between instanced corn vertex structs and standard material vertex structs
	This file handles all of the differences between prepass and regular vertex shader code using shaderdefs, 
	so by importing these types other shaders can completely ignore the problem and everything will just work.
*/

#if PREPASS == true
	#import bevy_pbr::prepass_io::Vertex
#else
	#import bevy_pbr::forward_io::Vertex
#endif

// vertex Program input
struct CornVertex {
    @builtin(instance_index) instance_index: u32,
	#if PREPASS == true
		@location(0) position: vec3<f32>,
		#ifdef VERTEX_UVS
			@location(1) uv: vec2<f32>,
		#endif
		#ifdef VERTEX_UVS_B
			@location(2) uv_b: vec2<f32>,
		#endif
		#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
			@location(3) normal: vec3<f32>,
			#ifdef VERTEX_TANGENTS
				@location(4) tangent: vec4<f32>,
			#endif
		#endif
		#ifdef SKINNED
			@location(5) joint_indices: vec4<u32>,
			@location(6) joint_weights: vec4<f32>,
		#endif
		#ifdef VERTEX_COLORS
			@location(7) color: vec4<f32>,
		#endif
	#else
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
	#endif
	#ifdef CORN_INSTANCED
        @location(8) offset_scale: vec4<f32>,
        @location(9) rotation: vec2<f32>,
        @location(10) id: vec2<u32>,
    #endif
	#ifdef MORPH_TARGETS
		@builtin(vertex_index) index: u32,
	#endif
}
// Struct which holds useful information. the same regardless of shaderdefs, so functions can act on the structure freely without worrying about what elements actually exist
struct CornData{
	instance_id: u32,
	position: vec3<f32>,
	normal: vec3<f32>,
	_padding: u32,
	tangent: vec4<f32>,
	offset: vec3<f32>,
	scale: f32,
	rotation: vec2<f32>,
	id: vec2<u32>
}
// Gets CornData from a CornVertex struct
fn extract_data(vertex: CornVertex) -> CornData{
	var out: CornData;
	out.instance_id = vertex.instance_index;
	#ifdef CORN_INSTANCED
		out.offset = vertex.offset_scale.xyz;
		out.scale = vertex.offset_scale.w;
		out.rotation = vertex.rotation;
		out.id = vertex.id;
	#endif
	#if PREPASS == true
		out.position = vertex.position;
		#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
			out.normal = vertex.normal;
			#ifdef VERTEX_TANGENTS
				out.tangent = vertex.tangent;
			#endif
		#endif
	#else
		#ifdef VERTEX_POSITIONS
			out.position = vertex.position;
		#endif
		#ifdef VERTEX_NORMALS
			out.normal = vertex.normal;
		#endif
		#ifdef VERTEX_TANGENTS
			out.tangent = vertex.tangent;
		#endif
	#endif
	return out;
}
// Converts a Corn Vertex Struct and a CornData struct to a Standard Material Vertex Struct.
fn to_standard_input(altered: CornData, remainder: CornVertex, instance_id: u32) -> Vertex{
	var out: Vertex;
	out.instance_index = instance_id;
	#if PREPASS == true
		out.position = altered.position;
		#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
			out.normal = altered.normal;
			#ifdef VERTEX_TANGENTS
				out.tangent = altered.tangent;
			#endif
		#endif
	#else
		#ifdef VERTEX_POSITIONS
			out.position = altered.position;
		#endif
		#ifdef VERTEX_NORMALS
			out.normal = altered.normal;
		#endif
		#ifdef VERTEX_TANGENTS
			out.tangent = altered.tangent;
		#endif
	#endif
	#ifdef VERTEX_UVS
		out.uv = instanced.uv;
	#endif
	#ifdef VERTEX_UVS_B
		out.uv_b = instanced.uv_b;
	#endif
	#ifdef VERTEX_COLORS
		out.color = instanced.color;
	#endif
	#ifdef SKINNED
		out.joint_indices = instanced.joint_indices;
		out.joint_weights = instanced.joint_weights;
	#endif
	#ifdef MORPH_TARGETS
		out.index = instanced.index;
	#endif
	return out;
}
