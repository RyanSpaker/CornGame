#define_import_path corn_game::corn

struct PerCornData {
  offset: vec3<f32>,
  scale: f32,
  rotation: vec2<f32>,
  //currently empty: could be used to flag certain per corn field data later
  uuid: u32,
  enabled: u32
}
struct Range {
  start: u32,
  length: u32,
  offset: u32,
  //defrag stores the new offset here
  stale_range: u32,
}
struct CornSettings {
  range: Range,
  origin_res_width: vec4<f32>,
  height_width_min: vec2<f32>,
  step: vec2<f32>
}

fn hash(value: u32) -> u32 {
    var state = value;
    state = state ^ 2747636419u;
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    return state;
}

fn randomFloat(value: u32) -> f32 {
    return f32(hash(value)) / 4294967295.0;
}