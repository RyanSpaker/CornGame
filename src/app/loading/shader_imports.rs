use bevy::{prelude::*, utils::HashMap};

#[derive(Default, Resource)]
pub struct ShaderImports{
    shaders: HashMap<String, Handle<Shader>>
}

pub struct ShaderImportPlugin;
impl Plugin for ShaderImportPlugin{
    fn build(&self, app: &mut App) {
        app.init_resource::<ShaderImports>().add_systems(Startup, load_corn_common_shader);
    }
}

/// Adds the shader imports to the app.
fn load_corn_common_shader(mut res: ResMut<ShaderImports>, assets: Res<AssetServer>){
    res.shaders.insert("corn_game::utils::random".to_string(), assets.load::<Shader>("shaders/random.wgsl"));
    res.shaders.insert("corn_game::corn_types".to_string(), assets.load::<Shader>("shaders/corn/corn_common.wgsl"));
    res.shaders.insert("bevy_pbr::prepass_vertex".to_string(), assets.load::<Shader>("shaders/bevy/prepass_vertex.wgsl"));
    res.shaders.insert("bevy_pbr::standard_vertex".to_string(), assets.load::<Shader>("shaders/bevy/standard_vertex.wgsl"));
    res.shaders.insert("bevy_pbr::fragment".to_string(), assets.load::<Shader>("shaders/bevy/fragment.wgsl"));
    res.shaders.insert("corn_game::rendering::wind".to_string(), assets.load::<Shader>("shaders/corn/render/wind.wgsl"));
    res.shaders.insert("corn_game::rendering::vertex_io".to_string(), assets.load::<Shader>("shaders/corn/render/vertex_io.wgsl"));
}

