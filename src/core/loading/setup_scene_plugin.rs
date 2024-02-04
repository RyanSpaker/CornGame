use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponentPlugin;
use crate::core::loading::LoadingTaskCount;
use crate::ecs::corn::asset::CornModel;
use crate::ecs::corn::field::cf_simple::SimpleRectangularCornField;
use crate::ecs::main_camera::MainCamera;
use crate::ecs::flycam::FlyCam;
use crate::util::DebugTag;

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
            .init_state::<SetupState>()
            .add_plugins(ExtractComponentPlugin::<DebugTag>::default())
            .add_systems(OnEnter(self.active_state), add_setup_scene_task)
            .add_systems(Update, (
                setup_scene
                    .run_if(corn_loaded.and_then(run_once())),
                remove_setup_scene_task
                    .run_if(in_state(SetupState::Finished).and_then(run_once()))
            ).run_if(in_state(self.active_state)));
    }
}

fn corn_loaded(corn: Res<CornModel>) -> bool {corn.loaded}

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
        transform: Transform::from_xyz(0.0, 2.5, -10.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        projection: Projection::Perspective(PerspectiveProjection{
            near: 0.1,
            far: 200.0,
            ..default()
        }),
        ..default()
    }, FlyCam, MainCamera{}));
    //Spawn Rest of Scene
    commands.spawn((
        SpatialBundle::INHERITED_IDENTITY,
        SimpleRectangularCornField::new(
            Vec3::ZERO, Vec2::ONE*500.0, 
            UVec2::new(1000, 1000), Vec2::new(0.9, 1.2), 0.2,
        )
    ));
    //box
    commands.spawn((PbrBundle{
        mesh: meshes.add(Mesh::from(shape::Box::new(1.0, 1.0, 1.0))),
        material: materials.add(StandardMaterial::from(Color::rgb(1.0, 1.0, 1.0))),
        transform: Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
        ..default()
    }, DebugTag{}));
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

    next_state.set(SetupState::Finished);
}
