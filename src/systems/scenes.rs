//! # SceneTransition Module:
//! ## Spawning: 
//! - SpawnScene(component): Send this event. Spawning happens during the next frame between StateTransition and Update.
//! - Spawn Entity: Spawn with (SceneEntity, Scene), automatically detected. OnSpawnScene is run, and Transform/Visibility auto added if needed.
//!
//! ## Despawning:
//! - Send DespawnScene(Entity): Despawns Specific Entity. OnDespawnScene is run, then SceneEntity is despawned
//! - Send DespawnSceneMany(component): Despawns all scenes matching component. OnDespawnScene is run many times, then SceneEntities are despawned
//! - Delete SceneEntity: Due to parent/child heiarchy entities autodespawned. Next frame OnDespawnScene is run, but scene already gone.
//!
//! ## OnSpawnScene, OnDespawnScene: 
//! Schedules run when scenes are spawned or despawned. Loading/Unloading code should go here.

use std::{hash::Hash, marker::PhantomData};
use bevy::{app::MainScheduleOrder, ecs::{entity::EntityHashSet, schedule::ScheduleLabel}, prelude::*, utils::hashbrown::HashSet};
use crate::util::observer_ext::*;

/// Trait used to identify components that are Scene identifying tags
pub trait CornScene: Component+Clone+PartialEq+Eq+std::fmt::Debug+std::hash::Hash{
    fn get_bundle(self) -> impl Bundle {return (SceneEntity, self)}
}
/// Tag Component for Scene Entities. 
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct SceneEntity;

/// Resource containing the current scene in OnDespawn and OnSpawn Schedules \
/// No meaning outside of the OnDespawn and OnSpawn Schedules
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Resource)]
pub struct CurrentScene(pub Entity);
/// Schedule run during SceneTransition Whenever the specified scene is spawned
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, ScheduleLabel)]
pub struct OnSpawnScene<S: CornScene>(pub S);
/// Schedule run during SceneTransition Whenever the specified scene is despawned
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, ScheduleLabel)]
pub struct OnDespawnScene<S: CornScene>(pub S);

/// Event Which Despawns all scenes of S
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
pub struct DespawnCornSceneMany<S: CornScene>(pub S);
/// Event for despawning a single scene
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct DespawnCornScene(pub Entity);
/// Event for spawning a single scene
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
pub struct SpawnCornScene<S: CornScene>(pub S);

/// Returns a run condition which only passes when a scene is loaded
pub fn scene_loaded<S: CornScene>(scene: S) -> impl FnMut(Query<&S>)->bool{
    move |query: Query<&S>| {
        query.iter().any(|s| *s==scene)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
struct SceneObservers;
impl ObserverParent for SceneObservers{fn get_name(&self) -> Name {Name::from("Scene Observers")}}

/// Event Sent whenever a Spawn Schedule should be run
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
struct RunSpawnSchedule<S: CornScene>(pub Entity, PhantomData<S>);
/// Event Sent whenever a Despawn Schedule should be run
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
struct RunDespawnSchedule<S: CornScene>(pub Entity, pub S);
/// Resurce which tracks loaded scenes of type S
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
struct SceneTracker<S: CornScene>{
    loaded: EntityHashSet,
    _phantom_data: PhantomData<S>
}
impl<S: CornScene> Default for SceneTracker<S>{fn default() -> Self {Self{loaded: EntityHashSet::default(), _phantom_data: PhantomData::default()}}}
impl<S: CornScene> SceneTracker<S>{
    /// Observer run whenever scenes are spawned. Sends events for the SceneTransition Schedule to react to. Also ensures invariants on Scene Entities
    fn observe_scene_spawn(
        trigger: Trigger<OnAdd, S>,
        res: Res<Self>,
        mut event_writer: EventWriter<RunSpawnSchedule<S>>,
        query: Query<Entity, Or<(Without<Transform>, Without<Visibility>, Without<SceneEntity>)>>,
        mut commands: Commands
    ){
        if res.loaded.contains(&trigger.entity()) {return;}
        event_writer.send(RunSpawnSchedule(trigger.entity(), PhantomData::default()));
        if let Ok(entity) = query.get(trigger.entity()){
            commands.entity(entity).insert_if_new((
                SceneEntity, Transform::default(), Visibility::Visible
            ));
        }
    }
    /// Observer run whenever scenes are despawned. Sends events for the SceneTransition Schedule to react to
    fn observe_scene_despawn(
        trigger: Trigger<OnRemove, S>,
        query: Query<&S>,
        res: Res<Self>,
        mut event_writer: EventWriter<RunDespawnSchedule<S>>
    ){
        let entity = trigger.entity();
        if !res.loaded.contains(&entity) {return;}
        let Ok(comp) = query.get(entity) else {return;};
        event_writer.send(RunDespawnSchedule(trigger.entity(), comp.to_owned()));
    }
}

/// Steps of the SceneTransition schedule
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
enum SceneTransitionSteps{
    GenericDespawnEventHandling,
    DespawnEventHandling,
    DespawnSchedules,
    Despawn,
    Spawn,
    SpawnSchedules
}
/// Schedule run after StateTransition which handles scene loading and unloading
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, ScheduleLabel)]
pub struct SceneTransition;
impl Plugin for SceneTransition{
    fn build(&self, app: &mut App) {
        app
            .register_type::<SceneEntity>()
            .register_type::<CurrentScene>()
            .register_type::<DespawnCornScene>()
            .register_type::<SceneTransitionSteps>()
            .register_type::<SceneTransition>()
            .register_type::<SceneObservers>()
            .init_schedule(Self);
        app.add_event::<DespawnCornScene>();

        let mut schedule_order = app.world_mut().resource_mut::<MainScheduleOrder>();
        schedule_order.insert_after(StateTransition, SceneTransition);
        let mut schedule = Schedule::new(SceneTransition);
        schedule.configure_sets((
            (
                SceneTransitionSteps::DespawnEventHandling, 
                SceneTransitionSteps::GenericDespawnEventHandling.run_if(on_event::<DespawnCornScene>)
            ),
            SceneTransitionSteps::DespawnSchedules,
            SceneTransitionSteps::Despawn,
            SceneTransitionSteps::Spawn,
            SceneTransitionSteps::SpawnSchedules
        ).chain());
        app.add_schedule(schedule);
    }
}
impl SceneTransition{
    fn handle_despawn_all_events<S: CornScene>(
        mut events: EventReader<DespawnCornSceneMany<S>>,
        query: Query<(Entity, &S)>,
        res: Res<SceneTracker<S>>,
        mut writer: EventWriter<RunDespawnSchedule<S>>
    ){
        let despawning: HashSet<S> = events.read().into_iter().map(|e| e.0.clone()).collect();
        // Send new events for all scenes that need to be despawned
        writer.send_batch(query.iter().filter_map(|(entity, scene)| {
            if !res.loaded.contains(&entity) {return None;}
            if despawning.contains(scene) {Some(RunDespawnSchedule(entity, scene.to_owned()))}
            else {None}
        }));
    }
    fn handle_despawn_single_events<S: CornScene>(
        mut event_reader: EventReader<DespawnCornScene>,
        query: Query<&S>,
        res: Res<SceneTracker<S>>,
        mut event_writer: EventWriter<RunDespawnSchedule<S>>
    ){
        event_writer.send_batch(event_reader.read().into_iter().filter_map(|DespawnCornScene(entity)| {
            if !res.loaded.contains(entity) {return None;}
            let Ok(comp) = query.get(*entity) else {return None;};
            Some(RunDespawnSchedule(*entity, comp.to_owned()))
        }));
    }

    fn get_despawn_entities<S: CornScene>(
        mut events: EventReader<RunDespawnSchedule<S>>
    )  -> Vec<(Entity, S)> {
        let mut entities = vec![];
        for RunDespawnSchedule(entity, comp) in events.read(){
            entities.push((*entity, comp.to_owned()));
        }
        return entities;
    }
    pub fn run_despawn_schedules<S: CornScene>(
        In(scenes): In<Vec<(Entity, S)>>,
        world: &mut World
    ){
        for (entity, component) in scenes{
            world.insert_resource(CurrentScene(entity));
            let _ = world.try_run_schedule(OnDespawnScene(component.clone()));
        }
    }

    fn despawn_scenes<S: CornScene>(
        mut commands: Commands, 
        mut res: ResMut<SceneTracker<S>>,
        mut events: EventReader<RunDespawnSchedule<S>>
    ) {
        for RunDespawnSchedule(entity, _) in events.read(){
            if res.loaded.remove(entity) {
                // A parent scene unloading could make entity not exist
                if let Some(com) = commands.get_entity(*entity) {com.despawn_recursive();}
            }
        }
    }

    fn spawn_scenes<S: CornScene>(
        mut commands: Commands,
        mut res: ResMut<SceneTracker<S>>,
        mut new_scenes: EventReader<SpawnCornScene<S>>,
        mut event_writer: EventWriter<RunSpawnSchedule<S>>
    ){
        let mut events = vec![];
        for SpawnCornScene(scene) in new_scenes.read(){
            let entity = commands.spawn((
                SceneEntity, scene.to_owned()
            )).id();
            res.loaded.insert(entity);
            events.push(RunSpawnSchedule(entity, PhantomData::default()));
        }
        event_writer.send_batch(events);
    }

    fn get_spawn_entities<S: CornScene>(
        mut events: EventReader<RunSpawnSchedule<S>>,
        mut tracker: ResMut<SceneTracker<S>>,
        query: Query<&S>
    ) -> Vec<(Entity, S)> {
        let mut scenes = vec![];
        for RunSpawnSchedule(entity, _) in events.read(){
            let Ok(comp) = query.get(*entity) else {continue;};
            tracker.loaded.insert(*entity);
            scenes.push((*entity, comp.to_owned()));
        }
        return scenes;
    }
    fn run_spawn_schedules<S: CornScene>(
        In(scenes): In<Vec<(Entity, S)>>,
        world: &mut World
    ){
        for (entity, component) in scenes{
            world.insert_resource(CurrentScene(entity));
            let _ = world.try_run_schedule(OnSpawnScene(component.clone()));
        }
    }
}

/// Creates a scene register command for app which setups scene transition systems for that scene
pub trait SceneTransitionApp{
    fn init_scene<S: CornScene>(&mut self) -> &mut Self;
}
impl SceneTransitionApp for App{
    fn init_scene<S: CornScene>(&mut self) -> &mut Self{
        if !self.is_plugin_added::<InitScenePlugin<S>>() {self.add_plugins(InitScenePlugin::<S>::default());}
        self
    }
}
#[derive(Debug, Clone)]
struct InitScenePlugin<S: CornScene>(pub PhantomData<S>);
impl<S: CornScene> Default for InitScenePlugin<S>{fn default() -> Self {Self(PhantomData::default())}}
impl<S: CornScene> Plugin for InitScenePlugin<S>{
    fn build(&self, app: &mut App) {
        app.add_event::<RunSpawnSchedule<S>>()
            .add_event::<RunDespawnSchedule<S>>()
            .init_resource::<SceneTracker<S>>()
            .add_observer_as(SceneTracker::<S>::observe_scene_spawn, SceneObservers)
            .add_observer_as(SceneTracker::<S>::observe_scene_despawn, SceneObservers)
            
            .add_event::<DespawnCornSceneMany<S>>()
            .add_event::<SpawnCornScene<S>>()
            
            .add_systems(SceneTransition, (
            SceneTransition::handle_despawn_all_events::<S>
                .run_if(on_event::<DespawnCornSceneMany<S>>)
                .in_set(SceneTransitionSteps::DespawnEventHandling),
            SceneTransition::handle_despawn_single_events::<S>
                .in_set(SceneTransitionSteps::GenericDespawnEventHandling),
            (
                SceneTransition::get_despawn_entities::<S>.pipe(SceneTransition::run_despawn_schedules::<S>)
                    .in_set(SceneTransitionSteps::DespawnSchedules),
                SceneTransition::despawn_scenes::<S>
                    .in_set(SceneTransitionSteps::Despawn)
            ).run_if(on_event::<RunDespawnSchedule<S>>),
            (
                SceneTransition::spawn_scenes::<S>
                    .in_set(SceneTransitionSteps::Spawn),
                SceneTransition::get_spawn_entities::<S>.pipe(SceneTransition::run_spawn_schedules::<S>)
                    .in_set(SceneTransitionSteps::SpawnSchedules)
            ).run_if(on_event::<RunSpawnSchedule<S>>)
        ));
    }
}


