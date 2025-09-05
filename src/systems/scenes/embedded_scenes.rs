use bevy::{ecs::{component::HookContext, world::DeferredWorld}, platform::collections::HashMap, prelude::*};
use crate::{systems::{scenes::{prelude::{ResolveScene, ScenePath, ScenePathResolveExt}, util::ButtonTriggerSwapParentScene, SceneObservers}, util::button::BackgroundSelectedColors}, util::observer_ext::ObserveAsAppExt, DevConfig};

/// Trait used to generalize embedded scene behaviour
pub trait EmbeddedScene: Component+PartialReflect{
    /// returns the name that refers to this scene. Its scenepath will be 'embedded#name'
    fn get_name(&self) -> String;
    /// Constructs the scene struct that will be used to spawn the scene
    fn create_scene(&self, world: &World) -> Scene;
}

/// Resource which holds cached scenes, created at app startup
#[derive(Default, Debug, Reflect, Resource)]
#[reflect(Resource)]
struct CachedEmbeddedScenes{
    #[reflect(ignore)]
    pub scenes: HashMap<ScenePath, (Box<dyn PartialReflect>, Handle<Scene>)>
}
impl CachedEmbeddedScenes{
    /// resolve function for getting a cached embedded scene component from a scene path
    fn embedded_path_resolver(In(scene_path): In<ScenePath>, cache: Res<Self>) -> Option<Box<dyn PartialReflect>>{
        if !scene_path.0.starts_with("embedded#") {return None;}
        cache.scenes.get(&scene_path)
            .map(|(comp, _)| comp.reflect_clone().ok()).flatten()
            .map(|reflect| reflect.into_partial_reflect())
    }
}

// OnAdd observer for EmbeddedScenes, creates the scene and adds SceneRoot and ScenePath to the entity
fn on_add_embedded_scene<S: EmbeddedScene>(
    trigger: Trigger<OnAdd, S>,
    query: Query<&S>,
    cache: Res<CachedEmbeddedScenes>,
    asset_server: Res<AssetServer>,
    world: &World,
    mut commands: Commands
){
    let Ok(scene_comp) = query.get(trigger.target()) else {return;};
    let path: ScenePath = ScenePath("embedded#".to_string()+scene_comp.get_name().as_str());
    // get cached scene if available, otherwise create and add a new scene
    let scene_handle = match cache.scenes.get(&path){
        Some((_, handle)) => {handle.clone()},
        None => {
            let scene = scene_comp.create_scene(world);
            asset_server.add(scene)
        }
    };
    // Add ScenePath and SceneRoot to entity
    commands.entity(trigger.target()).insert((SceneRoot(scene_handle), path));
}

pub trait EmbeddedSceneExt{
    fn register_embedded_scene<S: EmbeddedScene>(&mut self) -> &mut Self;
    fn cache_embedded_scene<S: EmbeddedScene>(&mut self, scene: S) -> &mut Self;
    fn list_scenes(&mut self) -> Vec<String>;
}
impl EmbeddedSceneExt for App{
    /// Sets up functionality to auto load embedded scenes when they are spawned
    fn register_embedded_scene<S: EmbeddedScene>(&mut self) -> &mut Self {
        self.add_observer_as(on_add_embedded_scene::<S>, SceneObservers)
    }
    /// Caches a scene components constructed Scene, so that it wont have to be created and stored during runtime. \
    /// Scenes should be cached late in the plugin stage to allow for resources to be initialized
    fn cache_embedded_scene<S: EmbeddedScene>(&mut self, scene: S) -> &mut Self {
        let asset_server = self.world().resource::<AssetServer>();
        let scene_handle = asset_server.add(scene.create_scene(self.world()));
        let cache = self.world_mut().resource_mut::<CachedEmbeddedScenes>();
        let path = ScenePath("embedded#".to_string()+scene.get_name().as_str());
        cache.into_inner().scenes.insert(path, (Box::new(scene), scene_handle));
        self
    }
    /// Returns a list of all currently cached embedded scenes
    fn list_scenes(&mut self) -> Vec<String> {
        self.world().resource::<CachedEmbeddedScenes>().scenes.keys().map(|key| key.0.clone()).collect()
    }
}

#[derive(Default, Debug, Clone)]
pub struct EmbeddedScenePlugin;
impl Plugin for EmbeddedScenePlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<CachedEmbeddedScenes>()
            .init_resource::<CachedEmbeddedScenes>();
        app
            .register_type::<LobbyScene>()
            .register_type::<MainMenuScene>()
            .register_type::<MainMenuSubScene>();
        app
            .register_embedded_scene::<LobbyScene>()
            .register_embedded_scene::<MainMenuScene>()
            .register_embedded_scene::<MainMenuSubScene>();
        app.register_type::<SpawnLobbyScenes>();
    }
    fn finish(&self, app: &mut App) {
        app
            .cache_embedded_scene(LobbyScene)
            .cache_embedded_scene(MainMenuScene)
            .cache_embedded_scene(MainMenuSubScene::Title)
            .cache_embedded_scene(MainMenuSubScene::Options)
            .cache_embedded_scene(MainMenuSubScene::Credits);
        app.register_scene_path_resolver(CachedEmbeddedScenes::embedded_path_resolver);
    }
}

/*
    Lobby and Main Menu:
*/

/// Component used by the Lobby scene to spawn CLI scenes
#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)] #[component(on_add=SpawnLobbyScenes::on_add)]
pub struct SpawnLobbyScenes(pub Vec<ScenePath>);
impl SpawnLobbyScenes{
    /// on_add hook which spawns the scene list on the specified entity, and then despawns this entity
    fn on_add(mut world: DeferredWorld, HookContext{entity, ..}: HookContext){
        let Some(SpawnLobbyScenes(scenes)) = world.get::<Self>(entity).cloned() else {return;};
        let Some(ChildOf(parent)) = world.get::<ChildOf>(entity).cloned() else {return;};
        world.commands().spawn_batch(scenes.into_iter().map(move |scene| {
            (ResolveScene(scene.0), ChildOf(parent))
        }));
        world.commands().entity(entity).despawn();
    }
}

#[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct LobbyScene;
impl EmbeddedScene for LobbyScene{
    fn get_name(&self) -> String {"lobby".into()}
    fn create_scene(&self, world: &World) -> Scene {
        let mut scene = World::new();
        let dev_config = world.resource::<DevConfig>();
        scene.spawn(SpawnLobbyScenes(dev_config.scenes.iter().map(|scene| scene.into()).collect()));
        Scene::new(scene)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)] 
#[require(Node{
    display: Display::Block, 
    width: Val::Percent(100.0), 
    height: Val::Percent(100.0), 
    ..default()
})]
pub struct MainMenuScene;
impl EmbeddedScene for MainMenuScene{
    fn get_name(&self) -> String {"main_menu".into()}
    fn create_scene(&self, _world: &World) -> Scene {
        let mut scene: World = World::new();
        scene.spawn(MainMenuSubScene::Title);
        Scene::new(scene)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
#[require(Node{
    width: Val::Percent(100.0), height: Val::Percent(100.0), 
    display: Display::Flex, flex_direction: FlexDirection::Column, 
    justify_content: JustifyContent::Center, align_items: AlignItems::Center,
    row_gap: Val::Px(5.0), ..Default::default()
})]
enum MainMenuSubScene{
    #[default] Title,
    Options,
    Credits
}
impl EmbeddedScene for MainMenuSubScene{
    fn get_name(&self) -> String {match self{
        Self::Title => "main_menu#title", 
        Self::Options => "main_menu#options", 
        Self::Credits => "main_menu#credits"
    }.into()}
    fn create_scene(&self, _world: &World) -> Scene {
        let mut world = World::new();
        match self{
        Self::Title => {
            world.spawn((
                Text::new("Corn Game"),
                TextColor(bevy::color::palettes::basic::GREEN.into()),
                BackgroundColor(Color::WHITE),
                TextFont{font_size: 100.0, ..Default::default()}
            ));
            world.spawn((
                Button,
                Text::new("Play"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene::new("embedded#main_menu".into(), "embedded#lobby".into())
            ));
            world.spawn((
                Button,
                Text::new("Options"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene::new("embedded#main_menu#title".into(), "embedded#main_menu#options".into())
            ));
            world.spawn((
                Button,
                Text::new("Credits"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene::new("embedded#main_menu#title".into(), "embedded#main_menu#credits".into())
            ));
        }
        Self::Options => {
            world.spawn((
                Text::new("Options"),
                TextColor(bevy::color::palettes::basic::BLACK.into()),
                BackgroundColor(Color::WHITE),
                TextFont{font_size: 100.0, ..Default::default()}
            ));
            world.spawn((
                Button,
                Text::new("Back"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene::new("embedded#main_menu#options".into(), "embedded#main_menu#title".into())
            ));
        }
        Self::Credits => {
            world.spawn((
                Text::new("Credits"),
                TextColor(bevy::color::palettes::basic::BLACK.into()),
                BackgroundColor(Color::WHITE),
                TextFont{font_size: 100.0, ..Default::default()}
            ));
            world.spawn((
                Button,
                Text::new("Back"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene::new("embedded#main_menu#credits".into(), "embedded#main_menu#title".into())
            ));
        }
        }
        Scene::new(world)
    }
}
