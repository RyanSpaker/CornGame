//! CODEX (GPT-6): standalone 0.20 lobby. Scene composition lives in assets/lobby.corn.
use bevy::{prelude::*, world_serialization::WorldAssetRoot};

mod computer;
mod browser;
mod controller;
// CODEX: loose props have independent movement filtering and material impact audio.
mod kick;
mod hold;
mod physics;
mod interaction;
mod monitor_material;
mod moon_shader;
mod voice_box;
// CODEX: post-it interaction and isolated paper/text rendering.
mod post_it;
mod post_it_text;
// CODEX: paper-only shader extension for darkness-sensitive editing assistance.
mod post_it_material;
// CODEX: the lobby and standalone lab share typography controls and the complete font catalog.
mod post_it_fonts;
mod post_it_editor;
// CODEX: Keep Blender marker placement and surface fitting separate from microphone behavior.
mod voice_box_placement;
mod scene;
mod dev;
mod ui;
// CODEX — GPT-6.1: reusable perception and hierarchy hulls, plus the configurable radio prop.
mod fitted_hull;
mod seen;
mod radio;
mod precompile;
mod post_it_warmup;
mod pipeline_watch;
mod pipeline_watch_arm;
// CODEX: native black background before the first frame, including i3 resizes.
#[cfg(target_os = "linux")]
mod startup_window;

// CODEX: native launches resolve from any working directory; browsers fetch assets beside the page.
fn main() {
    let mut app = App::new();
    let asset_dir = if cfg!(target_arch = "wasm32") { "assets" } else { concat!(env!("CARGO_MANIFEST_DIR"), "/assets") };
    // CLAUDE: textures load small first (assets/texture_sets, from tools/ktx2_convert), then sharp; its reader
    // wraps the default source, so it goes in before AssetPlugin
    let textures = texture_loading::TextureLoadingPlugin::new(asset_dir);
    textures.register_source(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_dir.into(),
                // CODEX: the static bundle uses default metadata; avoid a 404 probe per asset.
                meta_check: if cfg!(target_arch = "wasm32") {
                    bevy::asset::AssetMetaCheck::Never
                } else {
                    default()
                },
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Corn lobby — cornfile case study".into(),
                    resolution: (1280, 800).into(),
                    // CODEX: the deployment launcher owns the browser canvas and responsive layout.
                    canvas: cfg!(target_arch = "wasm32").then(|| "#lobby-canvas".into()),
                    fit_canvas_to_parent: cfg!(target_arch = "wasm32"),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((
        corn_scene::SceneFilePlugin {
            precedence_file: None,
            types_file: None,
        },
        corn_scripting::plugin,
        browser::plugin,
        scene::plugin,
        controller::plugin,
        interaction::plugin,
        computer::plugin,
        monitor_material::plugin,
        moon_shader::plugin,
        voice_box::plugin,
        voice_box_placement::plugin,
        // CLAUDE: colliders are authored in lobby.corn (ColliderConstructorHierarchy), only on what needs them
        physics::plugin,
        dev::plugin,
        hold::plugin,
        post_it::plugin,
    ))
    .add_plugins((fitted_hull::plugin, seen::plugin, radio::plugin, precompile::plugin, post_it_warmup::plugin, pipeline_watch::plugin, pipeline_watch_arm::plugin))
    // CLAUDE: after DefaultPlugins: its png/jpg/ktx2 loader must be registered after Bevy's
    .add_plugins(textures)
    .add_systems(Startup, load);
    corn_assets::register::<Image>(&mut app);
    corn_assets::register::<AudioSource>(&mut app);
    // CODEX: corn constructs the AudioPlayer newtype before patching its handle; Bevy omits this data.
    app.register_type_data::<Handle<AudioSource>, bevy::reflect::std_traits::ReflectDefault>();
    corn_assets::register_inline::<StandardMaterial>(&mut app);
    app.register_type::<AudioPlayer>()
        .register_type::<PlaybackSettings>();
    // CODEX: install after the primary Window entity exists, before winit starts.
    #[cfg(target_os = "linux")]
    app.add_plugins(startup_window::plugin);
    app.run();
}

// CODEX: the cornfile is the only scene entry point; no Skein plugin or extras interpretation.
pub(crate) fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(WorldAssetRoot(assets.load("lobby.corn")));
}
