use bevy::{asset::{io::AssetSourceId, AsAssetId, AssetPath}, ecs::{component::HookContext, world::DeferredWorld}, prelude::*};
use crate::systems::scenes::prelude::{ScenePath, ScenePathResolveExt};

// TODO: Ron Scenes

/// Component which represents a scene defined by a gltf file.
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[component(on_add = GltfScene::on_add_gltf_scene)]
pub struct GltfScene{
    pub file: String, 
    pub scene: String
}
impl GltfScene {
    /// Constructs a new GltfScene from a path#scene_name string
    pub fn new(path: impl AsRef<str>) -> Self{
        let mut path = path.as_ref().split("#");
        let file = path.next().unwrap_or_default().to_string();
        let scene = path.next().unwrap_or_default().to_string();
        Self{file, scene}
    }
    /// Gets the scene path describing this scene
    fn get_scene_path(&self) -> ScenePath{
        let path = self.file.clone();
        ScenePath(if self.scene.is_empty() {path} else {path+"#"+&self.scene})
    }
    /// on_add lifetime hook for gltfscene, 
    fn on_add_gltf_scene(mut world: DeferredWorld, HookContext{entity,..}:HookContext) {
        let gltf = world.get::<Self>(entity).unwrap();
        // load gltf
        let asset_server = world.resource::<AssetServer>();
        let handle: Handle<Gltf> = asset_server.load(&gltf.file);
        info!(%entity, path = format!("{}#{}", gltf.file, gltf.scene));
        let scene_path = gltf.get_scene_path();
        world.commands().entity(entity)
            .insert(GltfSceneHandle(handle))
            .insert_if_new(scene_path); // Add scene path automatically
    }
    /// Begins spawning loaded gltf scenes
    fn load_handler(
        asset_server: Res<AssetServer>,
        gltf: Res<Assets<Gltf>>,
        query: Query<(Entity, &Self, &GltfSceneHandle), (Without<SceneRoot>, Or<(Changed<GltfSceneHandle>, AssetChanged<GltfSceneHandle>)>)>,
        mut commands: Commands,
    ) {
        for (entity, l, h) in query.iter() {
            let loaded = asset_server.get_load_states(h.0.id());
            trace!(%entity, ?l, ?h, ?loaded);
            if asset_server.is_loaded(h.0.id()) {
                let gltf = gltf.get(h.0.id()).unwrap();
                let path = asset_server.get_path(h.0.id()).map(|p| p.to_string());
                debug!(path, "{:#?}", gltf);

                let scene = if l.scene != "" {
                    l.scene.clone().into_boxed_str()
                } else {
                    gltf.named_scenes.keys().next().unwrap().clone()
                };

                match gltf.named_scenes.get(&scene) {
                    Some(s) => {
                        commands.entity(entity).insert(SceneRoot(s.clone()));
                    }
                    None => error!(
                        path = l.file,
                        "does not have scene {}\navailable: {:?}",
                        l.scene,
                        gltf.named_scenes.keys().collect::<Vec<_>>()
                    ),
                }
            }
        }
    }
    /// Resolver to get a gltf scene from a scene path
    fn scene_path_resolver(In(scene_path): In<ScenePath>) -> Option<Box<dyn PartialReflect>>{
        let Ok(path) = AssetPath::try_parse(&scene_path.0) else {return None};
        if *path.source() == AssetSourceId::Default && path.get_full_extension().is_some_and(|ext| ext == "gltf" || ext == "glb") {
            return Some(Box::new(Self{
                file: path.path().to_string_lossy().to_string(),
                scene: path.label().unwrap_or_default().to_string()
            }));
        }
        return None;
    }
}

/// Component which holds the gltf handle for a gltf scene
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct GltfSceneHandle(pub Handle<Gltf>);
impl AsAssetId for GltfSceneHandle{
    type Asset = Gltf;
    fn as_asset_id(&self) -> AssetId<Self::Asset> {self.0.id()}
}

pub struct StoredScenePlugin;
impl Plugin for StoredScenePlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<GltfScene>()
            .register_type::<GltfSceneHandle>()
            .register_scene_path_resolver(GltfScene::scene_path_resolver)
            .add_systems(PostUpdate, GltfScene::load_handler);
    }
}
