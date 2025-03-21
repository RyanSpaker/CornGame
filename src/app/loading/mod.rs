/*
    Handles the Loading of the game,
    This includes the reading of the corn asset file
    it also includes the initial scene setup
*/
use std::{env::args, f32::consts::PI, ffi::OsStr, path::PathBuf};
use bevy::{core_pipeline::{bloom::{Bloom, BloomSettings}, tonemapping::Tonemapping}, prelude::*, render::{mesh::PlaneMeshBuilder, sync_world::SyncToRenderWorld}, state::state::FreelyMutableState};
use avian3d::prelude::*;
use lightyear::prelude::{AppComponentExt, ChannelDirection, ClientReplicate, ServerReplicate};
use serde::{Deserialize, Serialize};
use crate::{app::character::SpawnPlayerEvent, ecs::{corn::{asset::processing::CornAssetTransformer, field::{cf_image_carved::CornSensor, prelude::*}}, flycam::FlyCam, framerate::spawn_fps_text, main_camera::MainCamera}};

use super::character::Player;

#[derive(Resource, Default)]
pub struct LoadingTaskCount(pub usize);

#[derive(Resource, Default)]
pub struct LoadingExitState<T>(T) where T: States + Copy;

pub struct LoadGamePlugin<T> where T: States + Copy{
    active_state: T,
    exit_state: T
}
impl<T> LoadGamePlugin<T> where T: States + Copy{
    pub fn new(active_state: T, exit_state: T) -> Self {
        Self {active_state, exit_state}
    }
}
impl<T> Plugin for LoadGamePlugin<T> where T: FreelyMutableState + Copy{
    fn build(&self, app: &mut App) {
        app
            .insert_resource(LoadingTaskCount(1))
            .insert_resource(LoadingExitState::<T>(self.exit_state))
            .add_systems(
                Update, 
                (
                    schedule_exit_loading_state::<T>,
                    setup_scene.run_if(run_once),
                    spawn_fps_text.run_if(run_once)
                ).run_if(in_state(self.active_state))
            );//.add_systems(Startup, setup_scene.run_if(run_once())); This crashes, TODO why?

        app.add_systems(Update, TestCube::spawn_system);
        app.register_type::<TestCube>();
        app.register_component::<TestCube>(ChannelDirection::Bidirectional);

        app.insert_resource(UiScale(2.0));
    }
}

fn schedule_exit_loading_state<T>(
    task_count: Res<LoadingTaskCount>,
    mut next_state: ResMut<NextState<T>>,
    exit_state: Res<LoadingExitState<T>>
) where T: FreelyMutableState + Copy{
    if task_count.0 == 0{
        next_state.set(exit_state.0);
    }
}

fn setup_scene(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut task_count: ResMut<LoadingTaskCount>,
    asset_server: Res<AssetServer>,
    cli: Res<crate::Cli>
){
    //Spawn Camera
    commands.spawn((
        Camera3d::default(),
        
        //bloom
        Camera {
            hdr: true, // 1. HDR is required for bloom
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,

        Transform::from_xyz(0.0, 2.5, -10.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        Projection::Perspective(PerspectiveProjection{
            near: 0.1,
            far: 200.0,
            ..default()
        }),
        bevy_edge_detection::EdgeDetection::default(), //post-process shader
        Name::new("main_camera"), IsDefaultUiCamera, MainCamera, CornSensor::default()));

    // let my_gltf = asset_server.load("scenes/player.glb#Scene0");
    // commands.spawn(Player.bundle()).insert((
    //     // SceneRoot(my_gltf),
    //     Transform::from_xyz(0.0, 1.0, -10.0),
    // ));

    // //Spawn Rest of Scene
    // commands.spawn((
    //     Transform::default(),
    //     Visibility::default(),
    //     ImageCarvedHexagonalCornField::new(
    //         Vec3::ZERO, Vec2::ONE*75.0, 
    //         0.75, Vec2::new(0.9, 1.1), 0.2, 
    //         asset_server.load("textures/maze.png")
    //     ),
    // ));
    // commands.spawn((
    //     Mesh3d( meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
    //     MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(1.0, 0.0, 0.0)))),
    //     Transform::from_translation(Vec3::new(10.0, 0.5, 0.0)),
    // ));
    // //ground
    // commands.spawn((
    //     Mesh3d(meshes.add(PlaneMeshBuilder::new(Dir3::Y, Vec2::ONE*5000.0))),
    //     MeshMaterial3d(materials.add(StandardMaterial{
    //         base_color: Color::srgb(0.27, 0.19, 0.11),
    //         reflectance: 0.0,
    //         metallic: 0.0,
    //         ..Default::default()
    //     })),
    //     Collider::cuboid(1000.0, 0.01, 1000.0),
    //     RigidBody::Static
    // ));
    // light
    // commands.spawn(DirectionalLightBundle{
    //     directional_light: DirectionalLight { 
    //         illuminance: 15000.0,
    //         shadows_enabled: true, 
    //         ..default()
    //     },
    //     transform: Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, PI/4.0, -PI/4.0, 0.0)),
    //     ..default()
    // });
    use blenvy::*;
    
    commands.spawn((
        Name::new("default_floor"),
        Collider::cuboid(1000.0, 0.1, 1000.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Static,
    ));

    for path in cli.scenes.iter() {
        let path = path.strip_prefix("assets/").unwrap_or(&path);

        if path.extension() == Some(OsStr::new("glb")) {
        commands.spawn((
            BlueprintInfo::from_path(&path.to_str().unwrap()), //NOTE: I wish there was a language where I could just do path, and it would give a warning but I wouldn't have to do all this type munching
            SpawnBlueprint,
            GameWorldTag,
            RigidBody::Static // weird things happen if there are colliders with no rigid body
        ));
    }

    }

    commands.spawn(TestCube);

    task_count.0 -= 1;
}

/// Test object for debugging network / replication (or whatever)
#[derive(Component, Serialize, Deserialize, Reflect, PartialEq)]
#[reflect(Component)]
pub struct TestCube;
impl TestCube {
    fn spawn_system(
        query: Query<Entity, Added<Self>>,
        assets: ResMut<AssetServer>,
        mut commands: Commands
    ){
        for id in query.iter() {
            // XXX how can this fail
            commands.entity(id).insert((
                Name::new("test cube"),
                Mesh3d(assets.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
                MeshMaterial3d(assets.add(StandardMaterial::from(Color::rgb(1.0, 1.0, 1.0)))),
                Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
                //RigidBody::Dynamic,
                //Collider::cuboid(1.0, 1.0, 1.0),
                //AsyncCollider(ComputedCollider::ConvexHull),
                ServerReplicate::default(),
            ));
        }
    }
}

