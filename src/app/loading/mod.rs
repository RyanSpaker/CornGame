/*
    Handles the Loading of the game,
    This includes the reading of the corn asset file
    it also includes the initial scene setup
*/
use std::f32::consts::PI;
use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use bevy_replicon::core::replication_rules::Replication;
use bevy_xpbd_3d::prelude::*;
use serde::{Deserialize, Serialize};
use crate::ecs::{corn::{asset::processing::CornAssetTransformer, field::{cf_image_carved::CornSensor, prelude::*}}, flycam::FlyCam, framerate::spawn_fps_text, main_camera::MainCamera};

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
impl<T> Plugin for LoadGamePlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .insert_resource(LoadingTaskCount(1))
            .insert_resource(LoadingExitState::<T>(self.exit_state))
            .add_systems(
                Update, 
                (
                    schedule_exit_loading_state::<T>,
                    setup_scene.run_if(run_once()),
                    spawn_fps_text.run_if(run_once())
                ).run_if(in_state(self.active_state))
            );//.add_systems(Startup, setup_scene.run_if(run_once())); This crashes, TODO why?
    }
}

fn schedule_exit_loading_state<T>(
    task_count: Res<LoadingTaskCount>,
    mut next_state: ResMut<NextState<T>>,
    exit_state: Res<LoadingExitState<T>>
) where T: States + Copy{
    if task_count.0 == 0{
        next_state.set(exit_state.0);
    }
}

fn setup_scene(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut task_count: ResMut<LoadingTaskCount>,
    asset_server: Res<AssetServer>
){
    //Spawn Camera
    commands.spawn((Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.5, -10.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        projection: Projection::Perspective(PerspectiveProjection{
            near: 0.1,
            far: 200.0,
            ..default()
        }),
        ..default()
    }, /*FlyCam,*/ MainCamera, CornSensor::default()));

    let my_gltf = asset_server.load("scenes/player.glb#Scene0");
    commands.spawn(Player.bundle()).insert(SceneBundle {
        scene: my_gltf,
        transform: Transform::from_xyz(0.0, 1.0, -10.0),
        ..Default::default()
    });

    //Spawn Rest of Scene
    // commands.spawn((
    //     SpatialBundle::INHERITED_IDENTITY,

    //     ImageCarvedHexagonalCornField::new(
    //         Vec3::ZERO, Vec2::ONE*75.0, 
    //         0.75, Vec2::new(0.9, 1.1), 0.2, 
    //         asset_server.load("textures/maze.png")
    //     )
    // ));
    // commands.spawn(PbrBundle{
    //     mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
    //     material: materials.add(StandardMaterial::from(Color::rgb(1.0, 0.0, 0.0))),
    //     transform: Transform::from_translation(Vec3::new(10.0, 0.5, 0.0)),
    //     ..default()
    // });
    // //ground
    // commands.spawn(PbrBundle {
    //     mesh: meshes.add(PlaneMeshBuilder::new(Direction3d::Y, Vec2::ONE*5000.0)),
    //     material: materials.add(StandardMaterial{
    //         base_color: Color::rgb(0.27, 0.19, 0.11),
    //         reflectance: 0.0,
    //         metallic: 0.0,
    //         ..Default::default()
    //     }),
    //     ..default()
    // });
    // light
    commands.spawn(DirectionalLightBundle{
        directional_light: DirectionalLight { 
            illuminance: 15000.0,
            shadows_enabled: true, 
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, PI/4.0, -PI/4.0, 0.0)),
        ..default()
    });

    // note that we have to include the `Scene0` label
    let my_gltf = asset_server.load("scenes/cornmenu_min.glb#Scene0");
    commands.spawn(SceneBundle {
        scene: my_gltf,
        //transform: Transform::from_xyz(2.0, 0.0, -5.0), TODO play with this to test relative vs global coords in corn renderer are correct
        ..Default::default()
    }).insert((
        Collider::cuboid(1000.0, 0.01, 1000.0),
        RigidBody::Static
    ));

    commands.spawn(TestBox);

    task_count.0 -= 1;
}

#[derive(Component, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct TestBox;
impl TestBox {
    pub fn spawn(
        query: Query<(Entity, &Self), Added<Self>>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut commands: Commands
    ){
        for (id, _) in query.iter() {
            // XXX how can this fail
            commands.entity(id).insert((
                Name::new("test cube"),
                PbrBundle{
                    mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
                    material: materials.add(StandardMaterial::from(Color::rgb(1.0, 1.0, 1.0))),
                    transform: Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
                    ..default()
                },
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                //AsyncCollider(ComputedCollider::ConvexHull),
                Replication
            ));
        }
    }
}