//! This module contains all the per state functionality in the App.
//! Mainly consists of OnEnter(state) and OnExit(state) functions and spawning entities that are statescoped
pub mod lobby;
pub mod main_menu;
pub mod resolver;

use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    ecs::{component::HookContext, world::DeferredWorld},
    pbr::{ScreenSpaceReflections, VolumetricFog},
    prelude::*, scene::scene_spawner,
};
use lobby::LobbyScene;
use crate::{
    ecs::{cameras::MainCamera, corn::sensor::CornSensor, flycam::FlyCam, framerate::spawn_fps_text},
    systems::scenes::{CornScene, SceneTransitionApp},
    Cli,
};

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[component(on_add = LoadScene::on_add_load_scene)]
pub struct LoadScene {
    file: String,
    scene: String,
}
impl LoadScene {
    pub fn new<'a>(path: impl Into<&'a str>) -> Self {
        let mut path = path.into().split("#");
        Self {
            file: path.next().unwrap_or_default().to_string(),
            scene: path.next().unwrap_or_default().to_string(),
        }
    }
    fn on_add_load_scene(mut world: DeferredWorld, HookContext{entity,..}:HookContext) {
        let this = world.get::<LoadScene>(entity).unwrap();

        let asset_server = world.resource::<AssetServer>();
        let handle: Handle<Gltf> = asset_server.load(&this.file);

        info!(%entity, path = format!("{}#{}", this.file, this.scene));
        world.commands().entity(entity).insert(SceneGltf(handle));
    }
    fn load_handler(
        asset_server: Res<AssetServer>,
        gltf: Res<Assets<Gltf>>,
        query: Query<(Entity, &LoadScene, &SceneGltf), Without<SceneRoot>>,
        mut commands: Commands,
    ) {
        for (entity, l, h) in query.iter() {
            let loaded = asset_server.get_load_states(h.0.id());
            trace!(%entity, ?l, ?h, ?loaded);
            if asset_server.is_loaded(h.0.id()) {
                let gltf = gltf.get(h.0.id()).unwrap();
                let path = asset_server.get_path(h.0.id()).map(|p| p.to_string());
                debug!(path, "{:#?}", gltf);

                let scene = if l.scene != "" {
                    l.scene.clone().into_boxed_str()
                } else {
                    gltf.named_scenes.keys().next().unwrap().clone()
                };

                match gltf.named_scenes.get(&scene) {
                    Some(s) => {
                        commands.entity(entity).insert(SceneRoot(s.clone()));
                    }
                    None => error!(
                        path = l.file,
                        "does not have scene {}\navailable: {:?}",
                        l.scene,
                        gltf.named_scenes.keys().collect::<Vec<_>>()
                    ),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct SceneGltf(pub Handle<Gltf>);

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct FirstPersonScene;
impl CornScene for FirstPersonScene {
    fn get_bundle(self) -> impl Bundle {
        (self, Name::from("First Person Scene"))
    }
}

#[derive(Default, Debug, Clone)]
pub struct CornScenesPlugin;
impl Plugin for CornScenesPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<FirstPersonScene>()
            .init_scene::<FirstPersonScene>()
            .register_type::<LoadScene>()
            .register_type::<SceneGltf>()
            .add_systems(Startup, (spawn_global_entities, spawn_fps_text))
            .add_systems(
                SpawnScene, //PostUpdate causes falling through floor
                LoadScene::load_handler.before(scene_spawner),
            )
            .add_plugins((main_menu::MainMenuPlugin, lobby::LobbyPlugin, bevy_dog::plugin::DoGPlugin));
    }
}

fn spawn_global_entities(mut commands: Commands, cli: Res<Cli>) {
    let cam = MainCamera::spawn_main_camera(&mut commands);
    commands.entity(cam).insert((
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 2.5, -10.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        Projection::Perspective(PerspectiveProjection {
            near: 0.1,
            far: 200.0,
            ..default()
        }),
        // TODO need way to specify camera settings as asset, at commandline, or as part of scene
        // bevy_edge_detection::EdgeDetection::default(), //post-process shader
        // bevy_dog::settings::DoGSettings::OUTLINE_DITHER,
        // bevy_dog::settings::PassesSettings::default(),
        VolumetricFog {
            ambient_intensity: 0.0,
            ..default()
        },
        // ScreenSpaceReflections::default(), // problems on wasm
        CornSensor::default(),
        FlyCam,
        IsDefaultUiCamera,
    ));

    if cli.menu {
        commands.spawn(main_menu::MainMenuScene.get_bundle());
    } else if !cli.scenes.is_empty() || cli.lobby {
        commands.spawn(LobbyScene.get_bundle());
    }
    
    commands.insert_resource(UiScale(1.0));
}
