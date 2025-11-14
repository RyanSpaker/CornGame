#define_import_path corn_game::corn

struct PerCornData {
    offset: vec3<f32>,
    scale: f32,
    rotation: vec2<f32>,
  //currently empty: could be used to flag certain per corn field data later
    uuid: u32,
    enabled: u32
}

// Per corn data converted to this in the scan prepass.
struct VertexPerCornData {
    to_world: mat4x4<f32>
}

struct CornSettings {
    origin: vec3<f32>,
    width: u32,
    height_range: vec2<f32>,
    step: vec2<f32>,
    random_settings: vec2<f32>,
    uv_scale: vec2<f32>,
}