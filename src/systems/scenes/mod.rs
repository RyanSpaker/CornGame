pub mod transition;
pub mod stored;
pub mod embedded_scenes;

use avian3d::prelude::Collider;
use bevy::prelude::*;
use crate::{systems::scenes::util::{ForeignComponent, StaticComponent}, util::observer_ext::*};

pub mod prelude{
    pub use super::{
        transition::*, 
        stored::GltfScene, 
        embedded_scenes::{EmbeddedScene, EmbeddedSceneExt, LobbyScene, MainMenuScene},
        resolve::*,
        util::*,
        persist::*,
        scene_path_resolve::{ResolveScene, ScenePathResolveExt},
        initial_scenes::{GlobalScene, InitialScenes, InitialSceneExt},
        testing::prelude::*
    };
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
struct SceneObservers;
impl ObserverParent for SceneObservers{fn get_name(&self) -> Name {Name::from("Scene Observers")}}

pub struct CornScenePlugin;
impl Plugin for CornScenePlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((
            transition::SceneTracking, 
            persist::ScenePersistPlugin,
            scene_path_resolve::ScenePathResolvePlugin,
            stored::StoredScenePlugin, 
            embedded_scenes::EmbeddedScenePlugin, 
            initial_scenes::InitialScenePlugin,
            testing::TestScenePlugin
        ));
        app.register_type::<StaticComponent>();
    }
}

/// Code to help resolve entity relations in scenes to actual entitys
pub mod resolve{
    use bevy::{ecs::system::SystemParam, prelude::*, scene::SceneInstance};

    #[derive(Debug, Clone, Component, Reflect)]
    pub enum EntityPointer{
        SameSceneName(String)
    }

    #[derive(Debug, Clone, SystemParam)]
    pub struct EntityResolver<'w, 's> {
        children: Query<'w, 's, &'static Children>,
        parents: Query<'w, 's, &'static ChildOf>,
        scene: Query<'w, 's, Entity, With<SceneInstance>>, 
        name: Query<'w, 's, &'static Name>, // TODO perhaps require a marker component on the target to speed this up
    }
    impl<'w, 's> EntityResolver<'w, 's> {
        pub fn resolve(&self, start: Entity, pointer: &EntityPointer) -> Result<Entity, ResolutionError> {
            match pointer {
                EntityPointer::SameSceneName(name) => {
                    let scene_root = match self.parents.iter_ancestors(start).find(|e|self.scene.contains(*e)) {
                        Some(e) => e,
                        None => return Err(ResolutionError::NoSceneAncestor),
                    };

                    let name = Name::new(name.clone());
                    match self.children.iter_descendants(scene_root).find(|e| self.name.get(*e) == Ok(&name)){
                        Some(e) => Ok(e),
                        None => return Err(ResolutionError::NameNotFound{
                            root: Some(scene_root),
                            name: name.as_str().to_string()
                        })
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone, thiserror::Error)]
    pub enum ResolutionError{
        #[error("entity has no scene ancestor")]
        NoSceneAncestor,
        #[error("'{name}' not found under {root:?}")]
        NameNotFound{
            root: Option<Entity>,
            name: String,
        }
    }
}

///Utility functions for common scene operations. Ex Scene Swapping function used with button triggers
pub mod util{
    use bevy::{ecs::{component::HookContext, system::lifetimeless::Read, world::DeferredWorld}, prelude::*};
    use crate::systems::{scenes::prelude::{ResolveScene, ScenePath}, util::button::ButtonEvent};

    /// Component which can be used to register types from foreign crates that dont impl Reflect but need to be added to scenes
    #[derive(Debug, Component, Clone, Reflect)]
    #[reflect(Component, opaque, type_path = false)] #[component(on_add=ForeignComponent::<T>::on_add)]
    pub struct ForeignComponent<T: Component+Clone>{
        comp: T,
        primed: bool
    }
    /// TODO: Implement this type correctly
    impl<T: Component+Clone> TypePath for ForeignComponent<T>{
        fn type_path() -> &'static str {
            core::any::type_name::<ForeignComponent<T>>()
        }
    
        fn short_type_path() -> &'static str {
            core::any::type_name::<ForeignComponent<T>>()
        }
    }
    impl<T: Component+Clone> ForeignComponent<T>{
        fn new(comp: T) -> Self{Self{comp, primed: false}}
        fn on_add(mut world: DeferredWorld, HookContext{entity, ..}: HookContext){
            let Some(comp) = world.get_mut::<Self>(entity) else {return;};
            if !comp.primed {comp.into_inner().primed = true; return;}
            // spawn component
            world.commands().entity(entity).queue(|mut entity_world: EntityWorldMut| {
                let Some(comp) = entity_world.take::<Self>() else {return;};
                entity_world.insert(comp.comp);
            });
        }
    }
    pub trait AsForeignComponentExt{
        fn as_foreign(self) -> ForeignComponent<Self> where Self: Sized+Component+Clone;
    }
    impl<C: Sized+Clone+Component> AsForeignComponentExt for C{
        /// Wraps the component in ForeignComponent, allowing it to be type registered and added to scenes if the underlying type cannot be derive reflect
        fn as_foreign(self) -> ForeignComponent<Self>{
            ForeignComponent::new(self)
        }
    }

    /// Component which can be used to prevent on add hooks of components from firing when adding it to a scene. Delays the hooks by a single add.
    #[derive(Debug, Component, Reflect)] #[component(on_add=StaticComponent::on_add)]
    #[reflect(Component, opaque)]
    pub struct StaticComponent{
        components: Vec<Box<dyn PartialReflect>>,
        primed: bool
    }
    impl Clone for StaticComponent{
        fn clone(&self) -> Self {
            let components = self.components.iter().map(|reflect| reflect.to_dynamic() ).collect();
            Self{components, primed: self.primed}
        }
    }
    impl StaticComponent{
        /// Makes a static component vessel for a PartialReflect Component
        pub fn new(components: Vec<Box<dyn PartialReflect>>) -> Self{
            Self{components, primed: false}
        }
        /// on add hook for this component. First add primes, second add spawns the contained component
        fn on_add(mut world: DeferredWorld, HookContext {entity, ..}: HookContext) {
            let Some(comp) = world.get_mut::<Self>(entity) else {return;};
            if !comp.primed {comp.into_inner().primed = true; return;}
            // spawn component
            world.commands().entity(entity).queue(|mut entity_world: EntityWorldMut| {
                let Some(comp) = entity_world.take::<Self>() else {return;};
                for comp in comp.components.into_iter(){
                    entity_world.insert_reflect(comp);
                }
            });
        }
    }
    pub trait AsStaticComponentExt{
        fn as_static(self) -> StaticComponent;
    }
    impl<C: PartialReflect+Component> AsStaticComponentExt for C{
        /// Wraps the component in StaticComponent, allowing it to be added to a scene without triggering hooks
        fn as_static(self) -> StaticComponent {
            StaticComponent::new(vec![Box::new(self)])
        }
    }

    pub fn button_trigger(
        trigger: Trigger<ButtonEvent>
    ) -> Option<Entity>{
        match trigger.1{
            Interaction::Pressed => Some(trigger.0),
            _ => None
        }
    }

    pub fn swap_parent_scene<S1: Component+PartialEq, S2: Bundle+Clone>(parent_scene: S1, new_scene: S2)
    -> impl FnMut(In<Option<Entity>>, Query<Read<ChildOf>>, Query<Read<S1>>, Query<Read<Transform>>, Commands)
    {
        move |
            In(entity) : In<Option<Entity>>, 
            parents: Query<Read<ChildOf>>,
            target_scenes: Query<Read<S1>>,
            transforms: Query<Read<Transform>>,
            mut commands: Commands
        | {
            let Some(entity) = entity else {return;};
            // Find parent scene of the entity
            let Some(target_scene) = parents.iter_ancestors(entity)
                .find(|e| target_scenes.get(*e).is_ok_and(|s| *s==parent_scene)) 
            else {return;};
            // despawn parent scene
            commands.entity(target_scene).despawn();
            // spawn new scene. Parent the scene to the old parent scenes parent if applicable. Clone transform from old scene
            let mut new_scene_commands = commands.spawn(new_scene.clone());
            if let Ok(parent) = parents.get(target_scene) {
                new_scene_commands.insert(parent.clone());
            }
            if let Ok(transform) = transforms.get(target_scene) {
                new_scene_commands.insert_if_new(transform.to_owned());
            }
        }
    }

    pub fn swap_parent_scenepath(parent_scene: ScenePath, new_scene: ScenePath)
    -> impl FnMut(In<Option<Entity>>, Query<Read<ChildOf>>, Query<Read<ScenePath>>, Query<Read<Transform>>, Commands)
    {
        move |
            In(entity) : In<Option<Entity>>, 
            parents: Query<Read<ChildOf>>,
            target_scenes: Query<Read<ScenePath>>,
            transforms: Query<Read<Transform>>,
            mut commands: Commands
        | {
            let Some(entity) = entity else {return;};
            // Find parent scene of the entity
            let Some(target_scene) = parents.iter_ancestors(entity)
                .find(|e| target_scenes.get(*e).is_ok_and(|s| *s==parent_scene)) 
            else {return;};
            // despawn parent scene
            commands.entity(target_scene).despawn();
            // spawn new scene. Parent the scene to the old parent scenes parent if applicable. Clone transform from old scene
            let mut new_scene_commands = commands.spawn(ResolveScene(new_scene.0.clone()));
            if let Ok(parent) = parents.get(target_scene) {
                new_scene_commands.insert(parent.clone());
            }
            if let Ok(transform) = transforms.get(target_scene) {
                new_scene_commands.insert_if_new(transform.to_owned());
            }
        }
    }

    /// Component added to button ui entities. Converts to an observer on add. Used to add button observers to scenes
    #[derive(Debug, PartialEq, Reflect, Component)]
    #[reflect(Component)] #[component(on_add=ButtonTriggerSwapParentScene::<S1, S2>::on_add)]
    pub struct ButtonTriggerSwapParentScene<S1: Component+PartialEq, S2: Bundle+Clone>(pub S1, pub S2);
    impl<S1: Component+PartialEq, S2: Bundle+Clone> ButtonTriggerSwapParentScene<S1, S2>{
        pub fn new(parent: S1, swap: S2) -> Self {Self(parent, swap)}
        /// Component hook which replaces this component with an observer on_add.
        fn on_add(mut world: DeferredWorld, HookContext{entity, ..}: HookContext){
            world.commands().entity(entity).queue(|mut entity_world: EntityWorldMut| {
               let Some(Self(parent, swap)) = entity_world.take::<Self>() else {return;};
               entity_world.observe(button_trigger.pipe(swap_parent_scene(parent, swap)));
            });
        }
    }

}

/// Code to facilitate entities of scenes that need to persist after scene despawn, or need to be reparented outside of the scene
pub mod persist{
    use bevy::prelude::*;
    
    /// Tag component for entities that need to be kept on scene despawn and reparented to the current scenes parent scene.
    #[derive(Default, Debug, Clone, Copy, PartialEq, Reflect, Component)]
    #[reflect(Component)]
    pub struct PersistEntityToSceneParent;
    /// Tag component for entities that need to be kept on scene despawn, and reparented to the global scene.
    #[derive(Default, Debug, Clone, Copy, PartialEq, Reflect, Component)]
    #[reflect(Component)]
    pub struct PersistEntityToGlobalScene;

    /// Relationship for scene entities that are not parented to their scene, pointing to the scene they are detached from.
    #[derive(Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)] #[relationship(relationship_target=DetachedSceneEntities)]
    pub struct DetachedFromScene(pub Entity);
    /// Relationship target holding all entities that are detached from the scene but still part of it.
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)] #[relationship_target(relationship=DetachedFromScene, linked_spawn)]
    pub struct DetachedSceneEntities(Vec<Entity>);

    pub trait DespawnSceneExt{
        /// Despawns a scene entity by first detaching any persistent entities in the parent heirarchy
        fn despawn_scene(&mut self);
    }
    impl<'a> DespawnSceneExt for EntityCommands<'a>{
        /// Despawns a scene entity by first detaching any persistent entities in the parent heirarchy
        fn despawn_scene(&mut self) {
            self.queue_handled(despawn_scene_command, bevy::ecs::error::warn);
        }
    }
    /// Despawns a scene entity by first detaching any persistent entities in the parent heirarchy
    pub fn despawn_scene_command(mut world: EntityWorldMut){
        // Recurse through dependency tree to find persistent components.
        let root = world.id();
        // Finds descendents of root that need to persist during scene despawn
        fn find_persistent_entities(
            In(root): In<Entity>, 
            parents: Query<&Children>, 
            scenes: Query<(), With<SceneRoot>>,
            p2parent: Query<(), With<PersistEntityToSceneParent>>, 
            p2global: Query<(), With<PersistEntityToGlobalScene>>,
            detached: Query<&DetachedFromScene>
        ) -> (Vec<Entity>, Vec<Entity>, Vec<(Entity, Entity)>){
            if !parents.contains(root) {return (vec![], vec![], vec![]);}
            let mut global = vec![];
            let mut parent = vec![];
            let mut scene = vec![];
            // breadth first search of child entities. Searches direct scene entities first, then sub scene entities
            // Direct scene entities with PersistEntityToSceneParent survive
            let mut direct_scene_descent = vec![root];
            let mut sub_scene_descent = vec![];
            // stack based, direct_scene_descent contains entities with children that need to be searched
            // sub_scene_descent contains entities with children that need to be searched and are part of a sub scene
            while !direct_scene_descent.is_empty() {
                let current_entity = direct_scene_descent.pop().unwrap();
                for child in parents.get(current_entity).unwrap().iter(){
                    if let Ok(detach_scene) = detached.get(child) {
                        scene.push((child, detach_scene.0));
                    }
                    else if p2global.contains(child) {global.push(child);}
                    else if p2parent.contains(child) {parent.push(child);}
                    if parents.contains(child) {
                        if scenes.contains(child) {sub_scene_descent.push(child);}
                        else {direct_scene_descent.push(child);}
                    }
                }
            }
            // BFS of sub scene entities
            while !sub_scene_descent.is_empty(){
                let current_entity = sub_scene_descent.pop().unwrap();
                for child in parents.get(current_entity).unwrap().iter(){
                    if let Ok(detach_scene) = detached.get(child) {
                        scene.push((child, detach_scene.0));
                    }
                    else if p2global.contains(child) {global.push(child);}
                    // No p2parent since sub scene entities would reparent to a scene that is getting despawed anyway
                    if parents.contains(child) {sub_scene_descent.push(child);}
                }
            }
            (global, parent, scene)
        }
        // Finds the parent scene of an entity
        fn find_parent_scene(
            In(child): In<Entity>,
            scenes: Query<(), With<SceneRoot>>,
            parents: Query<&ChildOf>
        ) -> Option<Entity> {
            for parent in parents.iter_ancestors(child){
                if scenes.contains(parent) {return Some(parent);}
            }
            return None;
        }
        // mutable world edits. persists entities
        world.world_scope(|world| {
            let (to_global, to_parent, detached) = world.run_system_cached_with(find_persistent_entities, root).unwrap();
            // global entities have no parent
            for entity in to_global.into_iter(){
                world.entity_mut(entity).remove::<ChildOf>();
            }
            // detached entities need a new parent, and to remove their relationship
            for (child, new_parent) in detached.into_iter(){
                world.entity_mut(child).insert(ChildOf(new_parent)).remove::<DetachedFromScene>();
            }
            // parent scene entities need to be reparented to this scenes parent.
            // find this scenes parent
            let parent_scene = world.run_system_cached_with(find_parent_scene, root).unwrap();
            // Reparent to new scene or global scene
            for child in to_parent.into_iter(){
                if let Some(parent) = parent_scene{
                    world.entity_mut(child).remove::<PersistEntityToSceneParent>().insert(ChildOf(parent));
                }else{
                    world.entity_mut(child).remove::<(ChildOf, PersistEntityToSceneParent)>();
                }
            }
        });
        // despawn scene
        world.despawn();
    }

    pub struct ScenePersistPlugin;
    impl Plugin for ScenePersistPlugin{
        fn build(&self, app: &mut App) {
            app.register_type::<PersistEntityToSceneParent>()
                .register_type::<PersistEntityToGlobalScene>()
                .register_type::<DetachedFromScene>()
                .register_type::<DetachedSceneEntities>();
        }
    }
}

/// Code to resolve a scene path to a scene component that spawns the scene
pub mod scene_path_resolve{
    use bevy::{ecs::{component::HookContext, reflect::ReflectCommandExt, system::SystemId, world::DeferredWorld}, prelude::*};

    use crate::systems::scenes::prelude::ScenePath;

    // register functions that build components from the 
    // fn(ScenePath) -> Option<Box<dyn PartialReflect>>
    #[derive(Default, Resource)]
    struct RegisteredScenePathResolvers(Vec<SystemId<In<ScenePath>, Option<Box<dyn PartialReflect>>>>);
    
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)] #[component(on_add = ResolveScene::on_add)]
    pub struct ResolveScene(pub String);
    impl ResolveScene{
        /// lifecycle hook which adds the on_add_command for the entity when this component is added
        fn on_add(mut world: DeferredWorld, HookContext{entity,..}:HookContext){world.commands().entity(entity).queue(Self::on_add_command);}
        /// Entity command for resolving the scenepath
        fn on_add_command(mut world: EntityWorldMut){
            let entity = world.id();
            let Some(Self(path)) = world.get::<Self>() else {return;};
            let scenepath = ScenePath::from(path.as_str());
            let registered_resolvers = world.resource::<RegisteredScenePathResolvers>().0.clone();
            world.world_scope(|world| {
                for system_id in registered_resolvers.into_iter(){
                    if let Some(component) = world.run_system_with(system_id, scenepath.clone()).unwrap() {
                        world.commands().entity(entity).remove::<Self>().insert_reflect(component);
                    }
                }
            });
        }
    }

    pub trait ScenePathResolveExt{
        fn register_scene_path_resolver<A>(&mut self, resolver: impl IntoSystem<In<ScenePath>, Option<Box<dyn PartialReflect>>, A>+'static) -> &mut Self;
    }
    impl ScenePathResolveExt for App{
        fn register_scene_path_resolver<A>(&mut self, resolver: impl IntoSystem<In<ScenePath>, Option<Box<dyn PartialReflect>>, A>+'static) -> &mut Self {
            let id = self.world_mut().register_system_cached(resolver);
            self.world_mut().resource_mut::<RegisteredScenePathResolvers>().0.push(id);
            self
        }
    }

    pub struct ScenePathResolvePlugin;
    impl Plugin for ScenePathResolvePlugin{
        fn build(&self, app: &mut App) {
            app.init_resource::<RegisteredScenePathResolvers>()
                .register_type::<ResolveScene>();
        }
    }
}

/// Code to configure what scenes to spawn on startup
pub mod initial_scenes{
    //! # Initial Scenes API
    //! - Allows spawning resolved ScenePaths
    //! - By default, spawns main menu
    //! - Specifying startup scenes removes the main menu default spawn, and sets up other spawns
    //! - Repeated configuration concatenates spawns
    //! - Test framework with simple test blueprint scenes, and a way to register custom tests.
    //! - A way to print all available scenes before app run, after plugin setup. Include all tests

    use bevy::{core_pipeline::{bloom::Bloom, tonemapping::Tonemapping}, pbr::VolumetricFog, prelude::*};
    use crate::{ecs::{cameras::MainCamera, corn::sensor::CornSensor}, systems::scenes::prelude::{MainMenuScene, ResolveScene, ScenePath}};

    /// Resource which determines whether to spawn the global scene at startup. 
    #[derive(Debug, Clone, PartialEq, Reflect, Resource)]
    #[reflect(Resource)]
    pub struct GlobalScene(pub bool);
    impl GlobalScene{
        fn spawn_global_scene(res: Res<Self>, mut commands: Commands){
            if !res.0 {return;}
            let cam = commands.spawn(MainCamera::get_bundle()).id();
            commands.entity(cam).insert((
                Tonemapping::TonyMcMapface,
                Bloom::NATURAL,
                Transform::from_xyz(0.0, 2.5, -10.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
                Projection::Perspective(PerspectiveProjection {
                    near: 0.1,
                    far: 200.0,
                    ..default()
                }),
                // TODO need way to specify camera settings as asset, at commandline, or as part of scene
                // bevy_edge_detection::EdgeDetection::default(), //post-process shader
                // bevy_dog::settings::DoGSettings::OUTLINE_DITHER,
                // bevy_dog::settings::PassesSettings::default(),
                VolumetricFog {
                    ambient_intensity: 0.0,
                    ..default()
                },
                // ScreenSpaceReflections::default(), // problems on wasm
                CornSensor::default(),
                IsDefaultUiCamera,
            ));    
            commands.insert_resource(UiScale(1.0));
        }
    }

    /// Holds the initial scenes to spawn on startup
    /// Empty: Spawn Main
    /// Non-Empty: Spawn Scenes
    /// DNE: Spawn Nothing
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Resource)]
    #[reflect(Resource)]
    pub struct InitialScenes(pub Vec<ScenePath>);
    impl InitialScenes{
        /// System which runs during startup that spawns all initial scenes
        fn spawn_scenes(res: Res<Self>, mut commands: Commands){
            if res.0.is_empty(){
                commands.spawn(MainMenuScene);
            }
            for scene_path in res.0.iter(){
                commands.spawn(ResolveScene(scene_path.0.clone()));
            }
        }
    }

    pub trait InitialSceneExt{
        fn set_global_scene(&mut self, enable: bool) -> &mut Self;
        fn set_initial_scenes(&mut self, scenes: Vec<ScenePath>) -> &mut Self;
        fn add_initial_scenes(&mut self, scenes: Vec<ScenePath>) -> &mut Self;
        fn remove_initial_scenes(&mut self) -> &mut Self;
    }
    impl InitialSceneExt for App{
        fn set_global_scene(&mut self, enable: bool) -> &mut Self {
            self.world_mut().resource_mut::<GlobalScene>().0 = enable;
            self
        }
        fn set_initial_scenes(&mut self, scenes: Vec<ScenePath>) -> &mut Self {
            self.world_mut().resource_mut::<InitialScenes>().0 = scenes;
            self
        }
        fn add_initial_scenes(&mut self, scenes: Vec<ScenePath>) -> &mut Self {
            self.world_mut().resource_mut::<InitialScenes>().0.extend(scenes);
            self
        }
        fn remove_initial_scenes(&mut self) -> &mut Self {
            self.world_mut().remove_resource::<InitialScenes>();
            self
        }
    }

    pub struct InitialScenePlugin;
    impl Plugin for InitialScenePlugin{
        fn build(&self, app: &mut App) {
            app.register_type::<InitialScenes>().register_type::<GlobalScene>()
                .init_resource::<InitialScenes>().insert_resource(GlobalScene(true))
                .add_systems(Startup, (InitialScenes::spawn_scenes, GlobalScene::spawn_global_scene));
        }
    }

}

/// Code to create a simple testing framework for defining tests
pub mod testing{
    use bevy::{ecs::{component::HookContext, reflect::ReflectCommandExt, schedule::ScheduleLabel, world::DeferredWorld}, platform::collections::HashMap, prelude::*};
    use crate::{ecs::{cameras::MainCamera, flycam::FlyCam}, systems::{scenes::{initial_scenes::InitialSceneExt, prelude::{EmbeddedScene, EmbeddedSceneExt}}, util::default_resources::{SimpleMaterials, SimpleMeshes}}};

    pub mod prelude{
        pub use super::{TestStartup, TestUpdate, TestScene, TestRegisterExt, EmptyTest, TestDefaultRender};
    }

    /// Schedule for a test scene that runs once during Startup
    #[derive(Debug, Clone, PartialEq, Eq, Hash, ScheduleLabel)]
    pub struct TestStartup(pub String);
    /// Schedule for a test scene that runs each frame during Update
    #[derive(Debug, Clone, PartialEq, Eq, Hash, ScheduleLabel)]
    pub struct TestUpdate(pub String);

    /// Component representing a test scene. scenepath refers to the name of the test, cached by TestSceneCache resource
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)] #[component(on_add=TestScene::on_add)]
    pub struct TestScene(pub String);
    impl TestScene{
        /// lifecycle hook which adds the registered test component to the test scene when the test scene is added
        fn on_add(mut world: DeferredWorld, HookContext{entity,..}:HookContext){
            let test_name = world.get::<Self>(entity).unwrap().0.clone();
            let cache = world.resource::<TestSceneCache>();
            let Some(Ok(comp)) = cache.0.get(&test_name).map(|c| c.reflect_clone()) else {return;};
            world.commands().entity(entity).insert_reflect(comp);
        }
        /// System run during startup that runs any TestStartup schedules needed
        fn run_startup(world: &mut World){
            let tests: Vec<String> = world.query::<&Self>().iter(world).map(|t| t.0.clone()).collect();
            for test in tests{
                if let Err(err) = world.try_run_schedule(TestStartup(test)) {
                    warn!(?err);
                }
            }
        }
        /// System run during update that runs any TestUpdate schedules needed
        fn run_update(world: &mut World){
            let tests: Vec<String> = world.query::<&Self>().iter(world).map(|t| t.0.clone()).collect();
            for test in tests{
                if let Err(err) = world.try_run_schedule(TestUpdate(test)) {
                    warn!(?err);
                }
            }
        }
    }
    
    /// Stores registered test names and their spawned component
    #[derive(Default, Debug, Resource)]
    struct TestSceneCache(pub HashMap<String, Box<dyn PartialReflect>>);

    /// Resource added to the app when a test is active. Used to determine if the test schedules should be run or not
    #[derive(Default, Debug, Clone, Copy, PartialEq, Reflect, Resource)]
    #[reflect(Resource)]
    struct TestActive;

    pub trait TestRegisterExt{
        fn register_test(&mut self, name: String, test_environment: Box<dyn PartialReflect>) -> &mut Self;
        fn activate_tests(&mut self) -> &mut Self;
        fn activate_test(&mut self, name: String) -> &mut Self;
        fn list_tests(&mut self) -> Vec<String>;
    }
    impl TestRegisterExt for App{
        fn register_test(&mut self, name: String, test_environment: Box<dyn PartialReflect>) -> &mut Self {
            self.world_mut().resource_mut::<TestSceneCache>().0.insert(name.clone(), test_environment);
            self.add_schedule(Schedule::new(TestUpdate(name.clone())));
            self.add_schedule(Schedule::new(TestStartup(name.clone())))
        }
        fn activate_tests(&mut self) -> &mut Self {
            self.init_resource::<TestActive>()
        }
        fn activate_test(&mut self, name: String) -> &mut Self {
            self.activate_tests().set_initial_scenes(vec![]).set_global_scene(false);
            self.world_mut().spawn(TestScene(name));
            self
        }
        /// Returns a list of all registered tests
        fn list_tests(&mut self) -> Vec<String> {
            self.world().resource::<TestSceneCache>().0.keys().cloned().collect()
        }
    }

    pub struct TestScenePlugin;
    impl Plugin for TestScenePlugin{
        fn build(&self, app: &mut App) {
            app.register_type::<TestScene>()
                .register_type::<TestActive>()
                .register_type::<EmptyTest>()
                .register_type::<TestDefaultRender>()
                .register_embedded_scene::<TestDefaultRender>()
                .init_resource::<TestSceneCache>();
        }
        fn finish(&self, app: &mut App) {
            if app.world().get_resource::<TestActive>().is_some() {
                app.add_systems(Startup, TestScene::run_startup)
                    .add_systems(Update, TestScene::run_update);
            }
        }
    }

    // Default Test Configurations

    /// Simple test with no extra entities or components added
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)]
    pub struct EmptyTest;

    /// Simple test with a main camera, and optionally: flycam, directional light, floor plane, and cube
    #[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
    #[reflect(Component)]
    pub struct TestDefaultRender{
        pub flycam: bool,
        pub directional_light: bool,
        pub floor: bool,
        pub cube: bool
    }
    impl EmbeddedScene for TestDefaultRender{
        fn get_name(&self) -> String {"TestDefaultRender".to_string()}
        fn create_scene(&self, world: &World) -> Scene {
            let mut scene = World::new();
            if self.directional_light {
                scene.spawn(DirectionalLight::default());
            }
            let simple_meshes = world.resource::<SimpleMeshes>();
            let simple_mats = world.resource::<SimpleMaterials>();
            if self.floor {
                scene.spawn((
                    Name::from("Floor"), 
                    Mesh3d(simple_meshes.plane.clone()),
                    MeshMaterial3d(simple_mats.white.clone())
                ));
            }
            if self.cube{
                scene.spawn((
                    Name::from("Cube"), 
                    Mesh3d(simple_meshes.cube.clone()),
                    MeshMaterial3d(simple_mats.red.clone())
                ));
            }
            let cam = scene.spawn(MainCamera::get_bundle()).id();
            if self.flycam{
                scene.entity_mut(cam).insert(FlyCam);
            }
            Scene::new(scene)
        }
    }
}

/*
    # Scenes:
    - Use bevy's internal scene representation as an asset that is spawned
    - SceneRoot+SceneInstanceId represent a active scene
    - ScenePath uniquely identifies what scene it is, but is not technically necessary
    - GLTF scenes have a component type with ScenePath being the file_path#scene_name
    - Embedded scenes each have their own component with ScenePath being embedded#name

    # Scene Hierarchy
    - Scenes can contain other scenes in a tree like structure.
    - The top level is the global scene, which is all entities not in a scene.
    - Try to keep scene heirarchy and entity heiarchy parallel
*/
