//! CLAUDE (claude-opus-5-5): the pylon demo. `cargo run -p pylon` (run the binary directly if cargo run
//! can't find the GPU). Everything in the scene comes from assets/pylon.corn.
//!
//! `PYLON_TOUR=<dir>` walks a fixed route instead of taking input, saves screenshots into `<dir>` and
//! exits: a check that the demo works end to end (the script assets/dev/tour.js). `PYLON_TIMING=1`
//! measures loading and per-frame script cost instead (see timing.rs). `PYLON_SCENE=pylon2.corn` loads
//! another scene file from assets instead. `CORN_DEV=<port>` opens the remote console (see corn_scripting's
//! remote_console.rs; `cargo run -p corn_dev -- 'screenshot()'`).

use bevy::{
    image::{ImageAddressMode, ImageSamplerDescriptor},
    prelude::*,
    camera::Exposure,
    world_serialization::WorldAssetRoot,
};
use bevy_rich_text3d::{LoadFonts, Text3d, Text3dPlugin, Text3dStyle};
use corn_scene::{RegisterValueParser, SceneFilePlugin, Value};
use corn_scripting::RunScript;

mod timing;

fn main() {
    let repeat = ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    };
    let timing = std::env::var("PYLON_TIMING").is_ok();
    let log = bevy::log::LogPlugin {
        filter: format!("{},corn_scene={}", bevy::log::DEFAULT_FILTER, if timing { "debug" } else { "info" }),
        ..default()
    };
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(log)
            .set(AssetPlugin { file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(), ..default() })
            .set(ImagePlugin { default_sampler: repeat })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "The Pylon".into(),
                    // timing measures how fast frames can be made, not the display's rate
                    present_mode: if timing { bevy::window::PresentMode::AutoNoVsync } else { default() },
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((
        // the types file is for the editor (tree-sitter-corn/vscode)
        SceneFilePlugin { precedence_file: None, types_file: Some(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/scene_types.json").into()) },
        corn_scripting::plugin,
        pylon::plugin,
    ))
    // world space text (the sign), in a font that ships with the demo
    .add_plugins(Text3dPlugin {
        load_system_fonts: false,
        serif_family: "Liberation Sans".into(),
        sans_serif_family: "Liberation Sans".into(),
        ..default()
    })
    .insert_resource(LoadFonts { font_embedded: vec![include_bytes!("../assets/fonts/LiberationSans-Bold.ttf")], ..default() })
    .add_systems(Startup, |mut commands: Commands, assets: Res<AssetServer>| {
        let scene = std::env::var("PYLON_SCENE").unwrap_or_else(|_| "pylon.corn".into());
        commands.spawn(WorldAssetRoot(assets.load(scene)));
    });
    // handle fields scene files and scripts may fill
    corn_assets::register::<Image>(&mut app);
    corn_assets::register::<AudioSource>(&mut app);
    corn_assets::register_inline::<StandardMaterial>(&mut app);
    corn_assets::mesh::register(&mut app);
    // bevy doesn't register these for reflection
    app.register_type::<AudioPlayer>().register_type::<PlaybackSettings>();
    // `Text3d: "words";` in a scene file
    app.register_type::<Text3dStyle>().register_value_parser::<Text3d>(|value, _| match value {
        Value::Str(s) => Ok(Some(Text3d::new(s))),
        _ => Ok(None),
    });
    // `Exposure: -1.1;` (its ev100): bevy reflects it as opaque, without serde
    app.register_value_parser::<Exposure>(|value, _| match value {
        Value::Num { value, unit: None } => Ok(Some(Exposure { ev100: *value as f32 })),
        _ => Ok(None),
    });
    if timing {
        app.add_plugins(timing::plugin);
    }
    if let Ok(dir) = std::env::var("PYLON_TOUR") {
        // the tour runs while its entity lives
        app.add_systems(Startup, move |mut commands: Commands| {
            let tour = commands.spawn(Name::new("tour")).id();
            commands.queue(RunScript::new("dev/tour.js", "tour", tour).with_args(serde_json::json!({ "dir": dir })));
        });
    }
    app.run();
}
