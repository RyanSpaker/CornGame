use avian3d::prelude::{Collider, PhysicsTime, RigidBody};
use bevy::{ecs::{component::HookContext, world::DeferredWorld}, pbr::FogVolume, platform::collections::HashMap, prelude::*};
use crate::{ecs::{cameras::MainCamera, test_cube::TestCube}, systems::{scenes::prelude::*, util::{button::BackgroundSelectedColors, default_resources::{SimpleMaterial, SimpleMesh}}}, util::observer_ext::ObserveAsAppExt, DevConfig};

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
        self.add_observer_as(on_add_embedded_scene::<S>, super::SceneObservers)
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
        app
            .register_scene_path("lobby".into())
            .add_systems(Update, LobbyScene::on_spawn.in_set(SceneSpawnSet("lobby".into())));
        app
            .register_type::<ButtonTriggerSwapParentScene<MainMenuScene, LobbyScene>>()
            .register_type::<ButtonTriggerSwapParentScene<MainMenuSubScene, MainMenuSubScene>>();
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
        world.commands().spawn_batch(scenes.into_iter().map(move |scene| {(
            Name::from("Level from: ".to_string()+&scene.0), 
            ResolveScene(scene.0), 
            ChildOf(parent)
        )}));
        world.commands().entity(entity).despawn();
    }
}

#[derive(Default, Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct LobbyScene;
impl LobbyScene{
    fn on_spawn(
        mut ambient: ResMut<AmbientLight>,
        mut time: ResMut<Time<avian3d::prelude::Physics>>,
        mut cameras: Query<&mut Transform, With<MainCamera>>
    ){
        ambient.brightness = 0.2;
        time.pause();
        for mut trans in cameras.iter_mut() {
            *trans = Transform::from_xyz(0.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
        }
    }
}
impl EmbeddedScene for LobbyScene{
    fn get_name(&self) -> String {"lobby".into()}
    fn create_scene(&self, world: &World) -> Scene {
        let mut scene = World::new();
        // Spawn lobby elements
        scene.spawn((
            // TODO keep centered on player
            Name::from("Fog Volume"),
            FogVolume {
                density_factor: 0.0001,
                ..default()
            },
            Transform::from_scale(Vec3::splat(35.0)),
        ));
        scene.spawn(TestCube.as_static());
        scene.spawn((
            Name::from("Floor"),
            Transform::from_scale(Vec3::new(1000.0, 0.0, 1000.0)),
            StaticCommand::spawn_bundle((
                Collider::cuboid(1.0, 0.1, 1.0),
                RigidBody::Static
            )),
            StaticComponent::new(vec![
                SimpleMesh::Plane.to_dynamic(),
                SimpleMaterial::White.to_dynamic(),
            ]),
        ));

        // scene.spawn((
        //     Name::from("Box"),
        //     Mesh3d(shapes.cube.clone()),
        //     MeshMaterial3d(materials.red.clone()),
        // ));
        // scene.spawn((
        //     Sun,
        //     DirectionalLight{illuminance: 1000.0, ..Default::default()},
        //     Transform::from_translation(Vec3::NEG_ONE*2000.0).looking_at(Vec3::ZERO, Vec3::Y)
        // )).with_child((
        //     Transform::from_scale(Vec3::splat(45.0)),
        //     NoRotationChild, //TODO should make directional light the child instead
        //     GltfScene::new("models/sky.glb#sun")
        // ));
        // scene.spawn((
        //     Moon,
        //     DirectionalLight{illuminance: 30.0, ..default()},
        //     Transform::from_translation(Vec3::new(-1.0, 1.0, -2.0).normalize()*1000.0)
        //         .looking_at(Vec3::ZERO, Vec3::Y)
        //         .with_scale(Vec3::splat(30.0)),//scale does weird things here
        //     GltfScene::new("models/sky.glb#moon")
        // ));
        // scene.spawn((Transform::from_xyz(0.0, 500.0, 0.0).with_scale(Vec3::splat(10.0)), GltfScene::new("models/sky.glb#sky")));
        
        // Spawn specified scenes
        let dev_config = world.resource::<DevConfig>();
        scene.spawn(SpawnLobbyScenes(dev_config.scenes.iter().map(|scene| 
            scene.strip_prefix("assets/").unwrap_or(scene).into()
        ).collect()));
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
                ButtonTriggerSwapParentScene(MainMenuScene, LobbyScene).as_static()
            ));
            world.spawn((
                Button,
                Text::new("Options"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene(Self::Title, Self::Options).as_static()
            ));
            world.spawn((
                Button,
                Text::new("Credits"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
                ButtonTriggerSwapParentScene(Self::Title, Self::Credits).as_static()
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
                ButtonTriggerSwapParentScene(Self::Options, Self::Title).as_static()
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
                ButtonTriggerSwapParentScene(Self::Credits, Self::Title).as_static()
            ));
        }
        }
        Scene::new(world)
    }
}
