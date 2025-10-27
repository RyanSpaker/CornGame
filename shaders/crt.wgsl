// Modified from:
// https://github.com/hiulit/Godot-3-2D-CRT-Shader/blob/62052678cb84cc13ca6d54eea37527ad5d446ecb/crt_shader.shader#L1
// https://github.com/bevyengine/bevy/blob/latest/assets/shaders/post_processing.wgsl

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::maths::{PI, PI_2}
#import bevy_pbr::mesh_view_bindings::globals

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
struct PostProcessSettings {
    // const bool show_curvature = true;
    show_curvature: u32, // fake bool
    
    // const float curvature_x_amount  = float(6.0); 
    // const float curvature_y_amount  = float(4.0);
    curvature: vec2<f32>,

    // const vec4 corner_color = vec4(0.0, 0.0, 0.0, 1.0);
    corner_color: vec4<f32>,

    // const float vignette_opacity = 0.2;
    vignette_opacity: f32,

    // const float horizontal_scan_lines_amount = 180.0;
    horizontal_scan_lines_amount: f32,

    // const float horizontal_scan_lines_opacity = 0.2;
    horizontal_scan_lines_opacity: f32,

    // const float horizontal_scan_lines_velocity = 0.005;
    horizontal_scan_lines_velocity: f32,

    // const float vertical_scan_lines_amount = 370.0;
    vertical_scan_lines_amount: f32,
    
    // const float vertical_scan_lines_opacity = 1.0;
    vertical_scan_lines_opacity: f32,

    // const float boost = 1.2;
    boost: f32,

    // const float aberration_amount = 1.0;
    aberration_amount: f32,

    time: f32,

}
@group(0) @binding(2) var<uniform> settings: PostProcessSettings;


@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let screen_size = vec2<f32>(textureDimensions(screen_texture));

    // uv in the input image
	var screen_uv = in.uv;
    if (settings.show_curvature != 0) {
		screen_uv = screen_uv * 2.0 - 1.0;
		let offset = abs(screen_uv.yx) / settings.curvature;
		screen_uv = screen_uv + screen_uv * offset * offset;
		screen_uv = screen_uv * 0.5 + 0.5;
	}

	var color = textureSample(screen_texture, texture_sampler, screen_uv).rgb;

	if (settings.aberration_amount > 0.0) {
		let adjusted_amount = settings.aberration_amount / screen_size.x;
		color.r = textureSample(screen_texture, texture_sampler, vec2(screen_uv.x + adjusted_amount, screen_uv.y)).r;
		color.g = textureSample(screen_texture, texture_sampler, screen_uv).g;
		color.b = textureSample(screen_texture, texture_sampler, vec2(screen_uv.x - adjusted_amount, screen_uv.y)).b;
	}

	if (settings.vignette_opacity > 0.0) {
		var vignette = screen_uv.x * screen_uv.y * (1.0 - screen_uv.x) * (1.0 - screen_uv.y);
		vignette = clamp(pow((screen_size.x / 4.0) * vignette, settings.vignette_opacity), 0.0, 1.0);
		color *= vignette;
	}

	if (settings.horizontal_scan_lines_opacity > 0.0) {
		var s = sin((screen_uv.y + settings.time * settings.horizontal_scan_lines_velocity) * settings.horizontal_scan_lines_amount * PI * 2.0);
		s = (s * 0.5 + 0.5) * 0.9 + 0.1;
		let scan_line = vec4(vec3(pow(s, settings.horizontal_scan_lines_opacity)), 1.0);
		color *= scan_line.rgb;
	}

	if (settings.vertical_scan_lines_opacity > 0.0) {
		var s = sin(screen_uv.x * settings.vertical_scan_lines_amount * PI * 2.0);
		s = (s * 0.5 + 0.5) * 0.9 + 0.1;
		let scan_line = vec4(vec3(pow(s, settings.vertical_scan_lines_opacity)), 1.0);
		color *= scan_line.rgb;
	}
	
    color *= settings.boost;

	// Fill the blank space of the corners, left by the curvature, with black.
	if (screen_uv.x < 0.0 || screen_uv.x > 1.0 || screen_uv.y < 0.0 || screen_uv.y > 1.0) {
		color = settings.corner_color.rgb;
	}

	return vec4<f32>(color, 1.0);
}



