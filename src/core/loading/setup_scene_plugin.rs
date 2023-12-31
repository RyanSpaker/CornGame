use std::f32::consts::PI;

use bevy::prelude::*;
use crate::core::loading::LoadingTaskCount;
use crate::flycam::FlyCam;
use crate::prelude::corn_model::CornLoadState;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum SetupState{
    #[default]
    NotStarted,
    Working,
    Finished
}

pub struct SetupScenePlugin<T> where T: States + Copy{
    active_state: T
}
impl<T> SetupScenePlugin<T> where T: States + Copy{
    pub fn new(active_state: T) -> Self {
        Self {active_state}
    }
}
impl<T> Plugin for SetupScenePlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .add_state::<SetupState>()
            .add_systems(OnEnter(self.active_state), add_setup_scene_task)
            .add_systems(Update, (
                setup_scene
                    .run_if(in_state(CornLoadState::Loaded).and_then(run_once())),
                remove_setup_scene_task
                    .run_if(in_state(SetupState::Finished).and_then(run_once()))
            ).run_if(in_state(self.active_state)));
    }
}

fn add_setup_scene_task(
    mut task_count: ResMut<LoadingTaskCount>,
    mut next_state: ResMut<NextState<SetupState>>
){
    task_count.0 += 1;
    next_state.set(SetupState::Working);
}
fn remove_setup_scene_task(
    mut task_count: ResMut<LoadingTaskCount>
){
    task_count.0 -= 1;
}

fn setup_scene(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<SetupState>>
){
    //Spawn Camera
    commands.spawn((Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    }, FlyCam));
    //Spawn Rest of Scene
    //box
    commands.spawn(PbrBundle{
        mesh: meshes.add(shape::Box::new(1.0, 1.0, 1.0).into()),
        material: materials.add(Color::rgb(1.0, 1.0, 1.0).into()),
        transform: Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
        ..default()
    });
    //ground
    commands.spawn(PbrBundle {
        mesh: meshes.add(shape::Plane::from_size(50.0).into()),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
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

    next_state.set(SetupState::Finished);
}
