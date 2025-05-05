//! This module contains all the per state functionality in the App.
//! Mainly consists of OnEnter(state) and OnExit(state) functions and spawning entities that are statescoped
pub mod main_menu;
pub mod lobby;

use bevy::{core_pipeline::{bloom::Bloom, tonemapping::Tonemapping}, pbr::{ScreenSpaceReflections, VolumetricFog}, prelude::*};
use crate::{ecs::{cameras::MainCamera, corn::field::cf_image_carved::CornSensor, flycam::FlyCam, framerate::spawn_fps_text}, systems::scenes::{CornScene, SceneTransitionApp}};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct FirstPersonScene;
impl CornScene for FirstPersonScene{
    fn get_bundle(self) -> impl Bundle {
        (self, Name::from("First Person Scene"))
    }
}

#[derive(Default, Debug, Clone)]
pub struct CornScenesPlugin;
impl Plugin for CornScenesPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<FirstPersonScene>()
            .init_scene::<FirstPersonScene>()
            .add_systems(Startup, (
                spawn_global_entities,
                spawn_fps_text
            ))
            .add_plugins((
                main_menu::MainMenuPlugin,
                lobby::LobbyPlugin
            ));
    }
}

fn spawn_global_entities(mut commands: Commands) {
    let cam = MainCamera::spawn_main_camera(&mut commands);
    commands.entity(cam).insert((
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 2.5, -10.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        Projection::Perspective(PerspectiveProjection{
            near: 0.1,
            far: 200.0,
            ..default()
        }),
        // TODO need way to specify camera settings as asset, at commandline, or as part of scene
        // bevy_edge_detection::EdgeDetection::default(), //post-process shader
        VolumetricFog{
            ambient_intensity: 0.0,
            ..default()
        },
        ScreenSpaceReflections::default(),
        CornSensor::default(),
        FlyCam,
        IsDefaultUiCamera
    ));
    
    commands.spawn(main_menu::MainMenuScene.get_bundle());
    commands.insert_resource(UiScale(1.0));
}
