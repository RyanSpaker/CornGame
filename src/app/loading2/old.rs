/*
    Handles the Loading of the game,
    This includes the reading of the corn asset file
    it also includes the initial scene setup
*/
use bevy::{ecs::schedule::SystemConfigs, prelude::*};
/*
    Loading paradigm:
    
        Every Level has a list of dependencies required, defined when the LevelDependencies resource is created
        The resource tracks which dependencies are finished or not.

        When a level is loaded, the resource spawns LevelDendency Entities corresponding to each dependency that needs to be loaded or unloaded.

        The resource queries the entities for completion, until all tasks are done, deletes the entities, and sets the state to loaded

        Every dependency has a init, work, and is_finished system to run when loading.

        init is run once at the start of the loading sequence, work is run every time when the dependency is not finished, and is_finished 
        When entering the level load state, a resource is created, tracking the unloaded dependencies needed
        When that resource shows no more unloaded dependencies, the state is moved to an active one

        Dependecies are specified by the levels themselves

        Global dependecy tracking so that we can easily determine what needs to be loaded at any point

        Dependecy types:
            one shot functions
            setup-work-finish functions
            setup-wait-finish functions
            wait-work-wait-...-finish functions

            setup: system: runs once when starting to load
            work: system set: runs once every frame while loading
            is_finished: system, returns bool telling whether the work is finished

            Assets that need to be loaded
            Resources that need to be setup
            Systems with custom setup logic
            Functions that need to be run
            States that need to be entered
        
        Dependencies need to know when to run

        They need init, finish, and regular update functions

        

*/


/// Trait describing necessary behaviour for LevelDependencies
pub trait AsLevelDependency<F, Marker> where F: SystemParamFunction<Marker, Out = bool>
{
    /// Returns a system to run once when we just start loading the dependency
    fn setup_function() -> Option<SystemConfigs> {None}
    /// Returns a system to run once every frame when we are loading the dependency
    fn work_function() -> Option<SystemConfigs> {None}
    /// Returns a system to run once per frame, returning the progress of the dependency
    fn status_function() -> Option<F> {None}
}
impl<F, Marker, S> AsLevelDependency<F, Marker> for S
where
    S: IntoSystem<(), (), Marker>,
    F: SystemParamFunction<Marker, Out = bool>
{
    fn setup_function() -> Option<SystemConfigs> {
        Some(IntoSystem::into_system(S))
    }
}

/// A component representing a Dependency for the currently loading level. 
#[derive(Component)]
pub struct LevelDependency{
    /// Whether the dependency is finished
    pub finished: bool
}
/// A component containing a description for a Level's Dependency
pub struct DependencyDescription{
    /// Name of the dependency
    pub name: String,
    /// Description of the dependency
    pub description: String
}
/// A component containing the current progress level of the dependency
pub struct DependencyProgress{
    /// 0-1 percent of progress for the dependency
    pub percent: f32
}



use std::f32::consts::PI;
use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use crate::ecs::{corn::field::{cf_image_carved::CornSensor, prelude::*}, flycam::FlyCam, framerate::spawn_fps_text, main_camera::MainCamera};

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
    }, FlyCam, MainCamera, CornSensor::default() /* Fxaa::default() */));

    //Spawn Rest of Scene
    commands.spawn((
        SpatialBundle::INHERITED_IDENTITY,
        ImageCarvedHexagonalCornField::new(
            Vec3::ZERO, Vec2::ONE*512.0, 
            0.75, Vec2::new(0.9, 1.1), 0.2, 
            asset_server.load("textures/Maze_Large.png")
        ),

        /*
        ImageCarvedHexagonalCornField::new(
            Vec3::ZERO, Vec2::ONE*75.0, 
            0.75, Vec2::new(0.9, 1.1), 0.2, 
            asset_server.load("textures/maze.png")
        )*/
    ));
    //box
    commands.spawn(PbrBundle{
        mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        material: materials.add(StandardMaterial::from(Color::rgb(1.0, 1.0, 1.0))),
        transform: Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
        ..default()
    });
    commands.spawn(PbrBundle{
        mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        material: materials.add(StandardMaterial::from(Color::rgb(1.0, 0.0, 0.0))),
        transform: Transform::from_translation(Vec3::new(10.0, 0.5, 0.0)),
        ..default()
    });
    //ground
    commands.spawn(PbrBundle {
        mesh: meshes.add(PlaneMeshBuilder::new(Direction3d::Y, Vec2::ONE*5000.0)),
        material: materials.add(StandardMaterial{
            base_color: Color::rgb(0.27, 0.19, 0.11),
            reflectance: 0.0,
            metallic: 0.0,
            ..Default::default()
        }),
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
