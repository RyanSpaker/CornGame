#define_import_path corn_game::corn

struct PerCornData {
  offset: vec3<f32>,
  scale: f32,
  rotation: vec2<f32>,
  uuid: u32,
  enabled: u32
}
struct Range {
  start: u32,
  length: u32,
  id: u32,
  offset: u32,
}
struct CornSettings {
  origin_res_width: vec4<f32>,
  height_width_min: vec2<f32>,
  step: vec2<f32>
}