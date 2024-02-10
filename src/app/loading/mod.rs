/*
    Handles the Loading of the game,
    This includes the reading of the corn asset file
    it also includes the initial scene setup
*/
use std::f32::consts::PI;
use bevy::prelude::*;
use crate::ecs::{corn::field::prelude::*, flycam::FlyCam, framerate::spawn_fps_text, main_camera::MainCamera};

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
            );
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
    mut task_count: ResMut<LoadingTaskCount>
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
    }, FlyCam, MainCamera));
    //Spawn Rest of Scene
    commands.spawn((
        SpatialBundle::INHERITED_IDENTITY,
        SimpleHexagonalCornField::new(
            Vec3::ZERO, Vec2::ONE*500.0, 
            0.75, Vec2::new(0.9, 1.1), 0.2,
        )
    ));
    //box
    commands.spawn(PbrBundle{
        mesh: meshes.add(Mesh::from(shape::Box::new(1.0, 1.0, 1.0))),
        material: materials.add(StandardMaterial::from(Color::rgb(1.0, 1.0, 1.0))),
        transform: Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
        ..default()
    });
    commands.spawn(PbrBundle{
        mesh: meshes.add(Mesh::from(shape::Box::new(1.0, 1.0, 1.0))),
        material: materials.add(StandardMaterial::from(Color::rgb(1.0, 0.0, 0.0))),
        transform: Transform::from_translation(Vec3::new(10.0, 0.5, 0.0)),
        ..default()
    });
    //ground
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane::from_size(1000.0))),
        material: materials.add(StandardMaterial::from(Color::rgb(0.3, 0.5, 0.3))),
        ..default()
    });
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
    task_count.0 -= 1;
}
