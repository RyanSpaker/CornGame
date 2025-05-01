use std::{hash::Hash, marker::PhantomData};

use bevy::{ecs::{entity::EntityHashSet, schedule::ScheduleLabel}, prelude::*, utils::hashbrown::HashSet};

/*
    Spawning: 
    - 1: Send Event: No knowledge of which entity refers to that scene
    - 2: Spawn Entity with (SceneEntity, Scene): w

    Despawning:
    - 1: Send Event: Despawns all of specific type
    - 2: Send Event2: Despawns Specific Entity
    - 3: Delete Entity: Despawns any entity of the scene. With parent/child most entities should auto despawn. DOES NOT RUN SCHEDULES

    OnEnter/OnExit: 
    - Run once for every Spawn/Despawn,
    - CurrentScene resource holds the entity id of the current scene being spawned/despawned
    - OnExit: Runs before entities of the scene are deleted if possible.

    After spawning, send events for Spawned/Despawned, containing root entity id, for the scenes
*/

/// Trait used to identify components that are Scene identifying tags
pub trait Scene: Component+Clone+Eq+std::fmt::Debug+std::hash::Hash{}
/// Tag Component for Scene Entities. 
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct SceneEntity;

/// Resource containing the current scene in OnDespawn and OnSpawn Schedules \
/// No meaning outside of the OnDespawn and OnSpawn Schedules
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Resource)]
pub struct CurrentScene(pub Entity);
/// Schedule run during SceneTransition Whenever the specified scene is spawned
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, ScheduleLabel)]
pub struct OnSpawnScene<S: Scene>(pub S);
/// Schedule run during SceneTransition Whenever the specified scene is despawned
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, ScheduleLabel)]
pub struct OnDespawnScene<S: Scene>(pub S);

/// Event Which Despawns all scenes of S
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct DespawnSceneMany<S: Scene>(pub S);
/// Event for despawning a single scene
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct DespawnScene(pub Entity);
/// Event for spawning a single scene
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct SpawnScene<S: Scene>(pub S);


/// Event Sent whenever a Spawn Schedule should be run
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
struct RunSpawnSchedule<S: Scene>(pub Entity, PhantomData<S>);
/// Event Sent whenever a Despawn Schedule should be run
#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
struct RunDespawnSchedule<S: Scene>(pub Entity, pub S);
/// Resurce which tracks loaded scenes of type S
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
struct SceneTracker<S: Scene>{
    loaded: EntityHashSet,
    _phantom_data: PhantomData<S>
}
impl<S: Scene> Default for SceneTracker<S>{fn default() -> Self {Self{loaded: EntityHashSet::default(), _phantom_data: PhantomData::default()}}}
impl<S: Scene> SceneTracker<S>{
    /// Observer run whenever scenes are spawned. Sends events for the SceneTransition Schedule to react to
    fn observe_scene_spawn(
        trigger: Trigger<OnAdd, S>,
        res: Res<Self>,
        mut event_writer: EventWriter<RunSpawnSchedule<S>>
    ){
        if res.loaded.contains(&trigger.entity()) {return;}
        event_writer.send(RunSpawnSchedule(trigger.entity(), PhantomData::default()));
    }
    /// Observer run whenever scenes are spawned. Sends events for the SceneTransition Schedule to react to
    fn observe_scene_despawn(
        trigger: Trigger<OnAdd, S>,
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
        app.init_schedule(Self);
        app.add_event::<DespawnScene>();
        app.configure_sets(SceneTransition, (
            (
                SceneTransitionSteps::DespawnEventHandling, 
                SceneTransitionSteps::GenericDespawnEventHandling.run_if(on_event::<DespawnScene>)
            ),
            SceneTransitionSteps::DespawnSchedules,
            SceneTransitionSteps::Despawn,
            SceneTransitionSteps::Spawn,
            SceneTransitionSteps::SpawnSchedules
        ).chain());
    }
}
impl SceneTransition{
    fn handle_despawn_all_events<S: Scene>(
        mut events: EventReader<DespawnSceneMany<S>>,
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
    fn handle_despawn_single_events<S: Scene>(
        mut event_reader: EventReader<DespawnScene>,
        query: Query<&S>,
        res: Res<SceneTracker<S>>,
        mut event_writer: EventWriter<RunDespawnSchedule<S>>
    ){
        event_writer.send_batch(event_reader.read().into_iter().filter_map(|DespawnScene(entity)| {
            if !res.loaded.contains(entity) {return None;}
            let Ok(comp) = query.get(*entity) else {return None;};
            Some(RunDespawnSchedule(*entity, comp.to_owned()))
        }));
    }

    fn get_despawn_entities<S: Scene>(
        mut events: EventReader<RunDespawnSchedule<S>>
    )  -> Vec<(Entity, S)> {
        return events.read().into_iter().map(|e| (e.0, e.1.to_owned())).collect()
    }
    pub fn run_despawn_schedules<S: Scene>(
        In(scenes): In<Vec<(Entity, S)>>,
        world: &mut World
    ){
        for (entity, component) in scenes{
            world.insert_resource(CurrentScene(entity));
            world.run_schedule(OnDespawnScene(component.clone()));
        }
    }
    
    fn despawn_scenes<S: Scene>(
        mut commands: Commands, 
        mut res: ResMut<SceneTracker<S>>,
        mut events: EventReader<RunDespawnSchedule<S>>
    ) {
        for RunDespawnSchedule(entity, _) in events.read(){
            if res.loaded.remove(entity) {
                commands.entity(*entity).despawn_recursive();
            }
        }
    }


    fn spawn_scenes<S: Scene>(
        mut commands: Commands,
        mut res: ResMut<SceneTracker<S>>,
        mut new_scenes: EventReader<SpawnScene<S>>,
        mut event_writer: EventWriter<RunSpawnSchedule<S>>
    ){
        let mut events = vec![];
        for SpawnScene(scene) in new_scenes.read(){
            let entity = commands.spawn((
                SceneEntity, scene.to_owned()
            )).id();
            res.loaded.insert(entity);
            events.push(RunSpawnSchedule(entity, PhantomData::default()));
        }
        event_writer.send_batch(events);
    }
    fn add_scene_components<S: Scene>(
        mut events: EventReader<RunSpawnSchedule<S>>,
        mut commands: Commands
    ){
        for RunSpawnSchedule(entity, _) in events.read(){
            commands.entity(*entity).insert_if_new((
                Transform::default(),
                Visibility::Visible
            ));
        }
    }
    
    fn get_spawn_entities<S: Scene>(
        mut events: EventReader<RunSpawnSchedule<S>>,
        query: Query<&S>
    ) -> Vec<(Entity, S)> {
        let mut scenes = vec![];
        for RunSpawnSchedule(entity, _) in events.read(){
            let Ok(comp) = query.get(*entity) else {continue;};
            scenes.push((*entity, comp.to_owned()));
        }
        return scenes;
    }
    fn run_spawn_schedules<S: Scene>(
        In(scenes): In<Vec<(Entity, S)>>,
        world: &mut World
    ){
        for (entity, component) in scenes{
            world.insert_resource(CurrentScene(entity));
            world.run_schedule(OnSpawnScene(component.clone()));
        }
    }
}

/// Creates a scene register command for app which setups scene transition systems for that scene
pub trait SceneTransitionApp{
    fn init_scene<S: Scene>(&mut self) -> &mut Self;
}
impl SceneTransitionApp for App{
    fn init_scene<S: Scene>(&mut self) -> &mut Self{
        self.add_plugins(InitScenePlugin::<S>::default())
    }
}
#[derive(Debug, Clone)]
struct InitScenePlugin<S: Scene>(pub PhantomData<S>);
impl<S: Scene> Default for InitScenePlugin<S>{fn default() -> Self {Self(PhantomData::default())}}
impl<S: Scene> Plugin for InitScenePlugin<S>{
    fn build(&self, app: &mut App) {
        app.add_event::<RunSpawnSchedule<S>>()
            .add_event::<RunDespawnSchedule<S>>()
            .init_resource::<SceneTracker<S>>()
            .add_observer(SceneTracker::<S>::observe_scene_spawn)
            .add_observer(SceneTracker::<S>::observe_scene_despawn);

        app.add_event::<DespawnSceneMany<S>>()
            .add_event::<SpawnScene<S>>();

        app.add_systems(SceneTransition, (
            SceneTransition::handle_despawn_all_events::<S>
                .run_if(on_event::<DespawnSceneMany<S>>)
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
                SceneTransition::add_scene_components::<S>
                    .after(SceneTransition::spawn_scenes::<S>)
                    .in_set(SceneTransitionSteps::Spawn),
                SceneTransition::get_spawn_entities::<S>.pipe(SceneTransition::run_spawn_schedules::<S>)
                    .in_set(SceneTransitionSteps::SpawnSchedules)
            ).run_if(on_event::<RunSpawnSchedule<S>>)
        ));
    }
}


