use std::path::Path;

use bevy::{ecs::{component::HookContext, entity::EntityHashSet, schedule::ScheduleLabel, world::DeferredWorld}, platform::collections::HashMap, prelude::*, scene::SceneInstanceReady};
use crate::util::observer_ext::ObserveAsAppExt;

/// Component of scene entities that determine what scene they are. Needed for scene tracking mechanisms to work
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)] #[component(on_add=ScenePath::on_add)]
pub struct ScenePath(pub String);
impl ScenePath{
    /// Creates a name from the scene path if the entity does not already have one
    fn on_add(mut world: DeferredWorld, HookContext{entity, ..}: HookContext){
        let Some(path) = world.get::<Self>(entity) else {return;};
        let name = path.0.split("#").last().unwrap_or(&path.0).to_string();
        world.commands().entity(entity).insert_if_new(Name::from(format!("Embedded Scene: {}", name)));
    }
}
impl From<&str> for ScenePath{
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
impl From<&Path> for ScenePath{
    fn from(value: &Path) -> Self {
        Self(value.to_string_lossy().to_string())
    }
}
/// Resource which holds info about loaded scenes defined by scenepath's
#[derive(Default, Debug, Clone, PartialEq, Reflect, Resource)]
#[reflect(Resource)]
pub struct LoadedScenes{
    loaded: HashMap<ScenePath, EntityHashSet>
}
impl LoadedScenes{
    /// Observer which updates loaded scenes and sends events when scenes are spawned
    pub fn detect_spawn(
        trigger: Trigger<SceneInstanceReady>,
        query: Query<&ScenePath>,
        mut loaded: ResMut<Self>,
        mut event_writer: EventWriter<SceneSpawned>
    ){
        if let Ok(path) = query.get(trigger.target()){
            loaded.insert(path, trigger.target());
            event_writer.write(SceneSpawned(path.clone()));
        }
    }
    /// Observer which updates loaded scenes and sends events when scenes are despawned
    pub fn detect_despawn(
        trigger: Trigger<OnRemove, ScenePath>,
        query: Query<&ScenePath>,
        mut loaded: ResMut<Self>,
        mut event_writer: EventWriter<SceneDespawned>
    ){
        if let Ok(path) = query.get(trigger.target()){
            loaded.remove(path, trigger.target());
            event_writer.write(SceneDespawned(path.clone()));
        }
    }
    /// Sets a scenepath and entity as not loaded
    pub fn remove(&mut self, path: &ScenePath, scene: Entity){
        let Some(set) = self.loaded.get_mut(path) else {return;};
        set.remove(&scene);
    }
    /// Sets a scenepath and entity as loaded
    pub fn insert(&mut self, path: &ScenePath, scene: Entity){
        if let Some(set) = self.loaded.get_mut(path) {
            set.insert(scene);
        } else {
            self.loaded.insert(path.to_owned(), EntityHashSet::from([scene]));
        }
    }
    /// Returns whether any entities with the scene path are currently loaded
    pub fn is_loaded(&self, path: &ScenePath) -> bool {
        self.loaded.contains_key(path)
    }
}

/// Event sent when a scene with scenepath is spawned
#[derive(Default, Debug, Clone, PartialEq, Reflect, Event)]
pub struct SceneSpawned(pub ScenePath);
/// Event sent when a scene with scenepath is despawned
#[derive(Default, Debug, Clone, PartialEq, Reflect, Event)]
pub struct SceneDespawned(pub ScenePath);

/// Run condition which checks to make sure a scene is loaded
pub fn on_scene(path: ScenePath) -> impl FnMut(Res<LoadedScenes>) -> bool {
    move |loaded: Res<LoadedScenes>| {
        loaded.is_loaded(&path)
    }
}

/// System set which runs only when scenepath is loaded at least once
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct SceneSet(pub ScenePath);
impl SceneSet{
    /// Sets up this sceneset to run during the schedule with the correct rules
    pub fn register_set(&self, app: &mut App, schedule: impl ScheduleLabel) {
        app.configure_sets(schedule, self.clone().run_if(on_scene(self.0.clone())));
    }
}

/// System set which runs once every time a scene path is loaded
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct SceneSpawnSet(pub ScenePath);
impl SceneSpawnSet{
    /// Sets up the scene set with the correct run condition
    pub fn register_set(&self, app: &mut App, schedule: impl ScheduleLabel) {
        let path = self.0.clone();
        app.configure_sets(schedule, self.clone().run_if(move |mut event_reader: EventReader<SceneSpawned>| {
            for event in event_reader.read() {if event.0 == path {return true;}}
            false
        }));
    }
}

/// Sets up scene tracking to the app
#[derive(Debug, Default, Clone)]
pub struct SceneTracking;
impl Plugin for SceneTracking{
    fn build(&self, app: &mut App) {
        app.register_type::<ScenePath>()
            .register_type::<LoadedScenes>()
            .register_type::<SceneSpawned>()
            .register_type::<SceneDespawned>()
            .init_resource::<LoadedScenes>()
            .add_event::<SceneSpawned>()
            .add_event::<SceneDespawned>()
            .add_observer_as(LoadedScenes::detect_spawn, super::SceneObservers)
            .add_observer_as(LoadedScenes::detect_despawn, super::SceneObservers);
    }
}

/// Allows global configuration of SceneSets
pub trait SceneTrackingExt{
    fn register_scene_path(&mut self, path: ScenePath) -> &mut Self;
}
impl SceneTrackingExt for App{
    fn register_scene_path(&mut self, path: ScenePath) -> &mut Self {
        SceneSet(path.clone()).register_set(self, Update);
        SceneSpawnSet(path.clone()).register_set(self, Update);
        self
    }
}

// Generic system definition in trait:
// pub trait SceneDescriptor: Component+Clone{
//     type CheckParam: ReadOnlySystemParam + 'static;
//     type CheckItem: ReadOnlyQueryData;
//     /// Checks the scene entity to see if it has loaded this descriptors content. ie, GLTF scenes would check the gltf asset load state
//     fn check_loaded<'w, 's>(item: ROQueryItem<'w, Self::CheckItem>, params: SystemParamItem<'w, 's, Self::CheckParam>) -> bool;
// }
