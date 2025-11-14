//! This module contains all the per state functionality in the App.
//! Mainly consists of OnEnter(state) and OnExit(state) functions and spawning entities that are statescoped
pub mod lobby;
pub mod main_menu;
pub mod resolver;

use bevy::{
    core_pipeline::{bloom::Bloom, prepass::DepthPrepass, tonemapping::Tonemapping}, ecs::{component::HookContext, system::SystemMeta, world::DeferredWorld}, pbr::{Atmosphere, ScreenSpaceReflections, VolumetricFog, wireframe::{Wireframe, WireframeColor}}, prelude::*, render::view::RenderLayers, scene::scene_spawner
};
use bevy_editor_pls::default_windows::cameras::EDITOR_RENDER_LAYER;
use clap::Parser;
use lobby::LobbyScene;
use crate::{
    Cli, ecs::{cameras::MainCamera, corn::sensor::CornSensor, flycam::FlyCam, framerate::spawn_fps_text, menu_main::spawn_main_menu}, systems::{character::{SpawnPlayerEvent, SpawnPlayerItem}, scenes::{CornScene, SceneTransitionApp}}, util::register_system_named::SystemMap
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

        if Cli::parse().spatialaudio {
            // waiting untill 0.17
            //https://github.com/janhohenheim/bevy_steam_audio
            app.add_plugins((
                // SeedlingPlugin::default(),
                // // Add the SteamAudioPlugin to the app to enable Steam Audio functionality
                // SteamAudioPlugin::default(),
                // // Steam Audio still needs some scene backend to know how to build its 3D scene.
                // // Mesh3dSteamAudioScenePlugin does this by using all entities that hold both
                // // `Mesh3d` and `MeshMaterial3d`.
                // Mesh3dSteamAudioScenePlugin::default(),
            ));
        }
    }
}

fn spawn_global_entities(mut commands: Commands, cli: Res<Cli>, server: Res<AssetServer>) {
    let cam = MainCamera::spawn_main_camera(&mut commands);
    commands.entity(cam).insert((
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
        // ScreenSpaceReflections::default(), // problems on wasm
        CornSensor::default(),
        FlyCam,
        IsDefaultUiCamera,
    ));

    if !cli.simplecam {
        commands.entity(cam).insert((
            VolumetricFog {
                ambient_intensity: 0.0,
                ..default()
            },
            Tonemapping::TonyMcMapface,
            Bloom::NATURAL,
            // Atmosphere::EARTH, nice scattering but don't like sky appearance
            // AtmosphereEnvironmentMapLight::default(), //0.17
            DepthPrepass,
        ));
    }

    if cli.spatialaudio {
        let listener = SpatialListener::new(0.25);
        commands.entity(cam).insert((
            listener.clone(),
            children![
                // left ear indicator
                (
                    Name::new("left ear gizmo"),
                    Mesh3d(server.add(Mesh::from(Cuboid::new(0.1, 0.1, 0.1)))),
                    Wireframe,
                    WireframeColor{color: Color::Srgba(Srgba::BLUE)},
                    Transform::from_translation(listener.left_ear_offset),
                    RenderLayers::layer(EDITOR_RENDER_LAYER)
                ),
                // right ear indicator
                (
                    Name::new("right ear gizmo"),
                    Mesh3d(server.add(Mesh::from(Cuboid::new(0.1, 0.1, 0.1)))),
                    Wireframe,
                    WireframeColor{color: Color::Srgba(Srgba::RED)},
                    Transform::from_translation(listener.right_ear_offset),
                    RenderLayers::layer(EDITOR_RENDER_LAYER)
                )
            ],
        ));
    }

    if cli.menu {
        // commands.spawn(main_menu::MainMenuScene.get_bundle());
        commands.run_system_cached(spawn_main_menu);
    } else if !cli.scenes.is_empty() || cli.lobby {
        commands.spawn(LobbyScene.get_bundle());
    }

    for item in cli.item.iter() {
        commands.trigger(SpawnPlayerEvent::default());
        commands.trigger(SpawnPlayerItem(item.strip_prefix("assets/").unwrap_or(item.as_str()).to_string()));
    }

    if cli.list_systems {
        commands.queue(move |world: &mut World| {
            world.schedule_scope(crate::Cmds, |world, schedule| {
                if schedule.systems().is_err() {
                    schedule.initialize(world).unwrap();
                }

                schedule.systems().unwrap().for_each(|s|println!("{}", s.1.name()));
            });
        });
    }

    for system in cli.system.iter() {
        let system = system.to_string();
        commands.queue(move |world: &mut World| -> Result{
            match world.resource::<SystemMap>().get_system(&system) {
                Some(s) => {
                    info!("running system {}", s.name);
                    let _ = world.run_system(s.id)?;
                },
                None => {
                    error!("no system named {}", system);
                },
            }            
            Ok(())

            // can't seem to access the actual system as mut
            // world.schedule_scope(crate::Cmds, |world, schedule| -> Result{
            //     if schedule.systems().is_err() {
            //         schedule.initialize(world).unwrap();
            //     }

            //     fn match_path(value: &str, full_path: &str)->bool{
            //         full_path == value || full_path.ends_with(value)
            //     }   

            //     //note. idk if we need schedule_scope
            //     let node = match schedule.systems().unwrap().find(|s| match_path(system.as_str(), &s.1.name())){
            //         Some(s) => s.0,
            //         None => {
            //             error!("no system named {} in Cmd schedule", system);
            //             return Ok(());
            //         },
            //     };
            //     let system = schedule.graph_mut().systems.get_mut(node.index()).unwrap().get_mut().unwrap();
            //     system.run((), world)
            // })
        });
    }

    if cli.testcorn {
        commands.run_system_cached(crate::ecs::corn::test_field);
    }   
    
    commands.insert_resource(UiScale(1.0));
}
