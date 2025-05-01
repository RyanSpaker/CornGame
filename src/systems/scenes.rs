use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

/*
    Spawning: 
    - 1: Send Event: No knowledge of which entity refers to that scene
    - 2: Spawn Entity with (SceneEntity, Scene): Does not run Schedules

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

/// Event Which Despawns all scenes of S
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct DespawnSceneManyEvent<S: Scene>(pub S);
/// Event for despawning a single scene
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct DespawnSceneEvent(pub Entity);
/// Event for spawning a single scene
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct SpawnSceneEvent<S: Scene>(pub S);

/// Resource containing the current scene in OnDespawn and OnSpawn Schedules \
/// No meaning outside of the OnDespawn and OnSpawn Schedules
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Resource)]
pub struct CurrentScene(pub Entity);

/// Schedule run after StateTransition which handles scene loading and unloading
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, ScheduleLabel)]
pub struct SceneTransition;
impl Plugin for SceneTransition{
    fn build(&self, app: &mut App) {
        app.init_schedule(Self);
        app.add_event::<DespawnSceneEvent>();
        app.configure_sets(SceneTransition, (
            SceneTransitionSteps::EventHandling,
            SceneTransitionSteps::DespawnSchedules,
            SceneTransitionSteps::Despawn,
            SceneTransitionSteps::Spawn
        ).chain());
        app.add_systems(SceneTransition, SceneTransition::despawn_scenes
            .in_set(SceneTransitionSteps::Despawn)
            .run_if(on_event::<DespawnSceneEvent>));
    }
}
impl SceneTransition{
    pub fn handle_despawn_all_event<S: Scene>(
        mut events: EventReader<DespawnSceneManyEvent<S>>,
        query: Query<(Entity, &S), With<SceneEntity>>,
        mut writer: EventWriter<DespawnSceneEvent>
    ){
        let despawning: Vec<S> = events.read().into_iter().map(|e| e.0.clone()).collect();
        // Send new events for all scenes that need to be despawned
        writer.send_batch(query.iter().filter_map(|(entity, scene)| {
            if despawning.iter().any(|s| s==scene) {
                Some(DespawnSceneEvent(entity))
            } else {None}
        }));
    }
    pub fn get_despawn_entities<S: Scene>(
        mut events: EventReader<DespawnSceneEvent>
    )  -> Vec<Entity> {
        return events.read().into_iter().map(|e| e.0).collect()
    }
    pub fn run_despawn_schedules<S: Scene>(
        In(entities): In<Vec<Entity>>,
        world: &mut World
    ){
        let mut events = vec![];
        for entity in entities{
            let Some(component) = world.get::<S>(entity).cloned() else {continue;};
            world.insert_resource(CurrentScene(entity));
            world.run_schedule(OnDespawnScene(component.clone()));
            events.push(DespawnedScene(entity, component));
        }
        world.send_event_batch(events);
    }
    pub fn despawn_scenes(
        mut commands: Commands, 
        mut events: EventReader<DespawnSceneEvent>
    ) {
        for entity in events.read().map(|e| e.0){
            commands.entity(entity).despawn_recursive();
        }
    }
    pub fn spawn_scenes<S: Scene>(
        mut commands: Commands,
        mut events: EventReader<SpawnSceneEvent<S>>
    ){
        let scenes: Vec<_> = events.read().map(|e| (
            SceneEntity, 
            e.0.clone(),
            Transform::default()
        )).collect();
        commands.spawn_batch(scenes);
    }
    pub fn get_spawn_entities<S: Scene>(
        query: Query<Entity, (Added<SceneEntity>, With<S>)>
    ) -> Vec<Entity> {
        query.into_iter().collect()
    }
    pub fn run_spawn_schedules<S: Scene>(
        In(entities): In<Vec<Entity>>,
        world: &mut World
    ){
        let mut events = vec![];
        for entity in entities{
            let Some(component) = world.get::<S>(entity).cloned() else {continue;};
            world.insert_resource(CurrentScene(entity));
            world.run_schedule(OnSpawnScene(component.clone()));
            events.push(SpawnedScene(entity, component));
        }
        world.send_event_batch(events);
    }
}

/// Steps of the SceneTransition schedule
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub enum SceneTransitionSteps{
    EventHandling,
    DespawnSchedules,
    Despawn,
    Spawn,
    SpawnSchedules
}

pub trait SceneTransitionApp{
    fn init_scene<S: Scene>(&mut self) -> &mut Self;
}
impl SceneTransitionApp for App{
    fn init_scene<S: Scene>(&mut self) -> &mut Self{
        self.add_event::<DespawnSceneManyEvent<S>>();
        self.add_event::<SpawnSceneEvent<S>>();
        self.add_event::<SpawnedScene<S>>();
        self.add_event::<DespawnedScene<S>>();
        self.add_systems(SceneTransition, (
            SceneTransition::handle_despawn_all_event::<S>
                .in_set(SceneTransitionSteps::EventHandling)
                .run_if(on_event::<DespawnSceneManyEvent<S>>),
            SceneTransition::get_despawn_entities::<S>.pipe(SceneTransition::run_despawn_schedules::<S>)
                .in_set(SceneTransitionSteps::DespawnSchedules)
                .run_if(on_event::<DespawnSceneEvent>),
            (
                SceneTransition::spawn_scenes::<S>
                    .in_set(SceneTransitionSteps::Spawn),
                SceneTransition::get_spawn_entities::<S>.pipe(SceneTransition::run_spawn_schedules::<S>)
                    .in_set(SceneTransitionSteps::SpawnSchedules)
            ).run_if(on_event::<SpawnSceneEvent<S>>)
        ))
    }
}

/// Schedule run during SceneTransition Whenever the specified scene is spawned
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, ScheduleLabel)]
pub struct OnSpawnScene<S: Scene>(pub S);
/// Schedule run during SceneTransition Whenever the specified scene is despawned
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, ScheduleLabel)]
pub struct OnDespawnScene<S: Scene>(pub S);

/// Event Sent whenever a scene is spawned. Contains the entity of the SceneEntity
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub struct SpawnedScene<S: Scene>(pub Entity, pub S);
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
/// Event Sent whenever a scene is despawned. Contains the entity of the now gone SceneEntity
pub struct DespawnedScene<S: Scene>(pub Entity, pub S);
