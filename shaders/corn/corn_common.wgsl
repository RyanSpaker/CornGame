#define_import_path corn_game::corn

struct PerCornData {
  offset: vec3<f32>,
  scale: f32,
  rotation: vec2<f32>,
  //currently empty: could be used to flag certain per corn field data later
  uuid: u32,
  enabled: u32
}

struct DefragRange {
  // Start of the range in the old buffer
  start: u32,
  // length of the range in the old buffer
  length: u32,
  // Instance offset of teh current range in the old buffer
  instance_offset: u32,
  // Index offset of the current field in the new buffer
  field_offset: u32,
}

struct StaleRange {
  // Start of the range in the buffer
  start: u32,
  // length of the range in the buffer
  length: u32,
  _padding: vec2<u32>,
}

struct SimpleInitRange{
  start: u32,
  length: u32,
  instance_offset: u32,
  _padding: u32
}

struct CornSettings {
  range: SimpleInitRange,
  origin_res_width: vec4<f32>,
  height_width_min: vec2<f32>,
  step: vec2<f32>,
  random_settings: vec4<f32>
}