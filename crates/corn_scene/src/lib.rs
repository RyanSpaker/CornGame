//! CLAUDE (claude-opus-5-5): the scene format: css-like `.corn` files that build a scene asset
//! (a `WorldAsset`) at load time: spawning entities, including other scenes (gltf, scene files), and
//! patching reflected components onto them. Spawn the file with `WorldAssetRoot(assets.load("x.corn"))`.
//!
//! ```css
//! use corn_game;
//!
//! script { const lamps = world.get_resource("Settings").lamps; }
//!
//! .static-box { RigidBody: Static; ColliderConstructor: Cuboid { x_length: 2 }; }
//! asset glow: StandardMaterial { base_color: #ffcc88; unlit: true; }
//! asset post: Mesh ${ mesh().add(Cylinder(0.1, 3), { translate: [0, 1.5, 0] }).add(Sphere(0.3), { translate: [0, 3, 0] }) };
//!
//! include "scenes/lobby.gltf#Scene0" {      /* copied in, then the body applies */
//!   #skybox { Transform.scale: [3000, 3000, 3000]; }
//!   #Cube\.001 { class: static-box; }
//!   #lamp > Mesh3d { MeshMaterial3d<StandardMaterial>: glow; Visibility: ${ lamps ? "Inherit" : "Hidden" }; }
//! }
//! spawn camera {
//!   include "cameras.corn#simplecam";
//!   Transform.translation: [0, 2, 8];
//!   on Pointer<Click>: ${ insert(e, "Shake", { time: 0.5 }); }
//! }
//! ```
//!
//! The file's scene has a root entity ([`SceneFileRoot`]) that its top-level items apply to; spawning
//! the scene makes it a child of the `WorldAssetRoot` entity. Its labeled assets: each top-level `spawn`
//! (`x.corn#camera`, a scene of its own), each `asset` (`x.corn#glow`), and with scripts the
//! module (`x.corn#$script`).
//!
//! Three ways to put a gltf and a `.corn` file together:
//! - the file owns the gltf: `include "level.gltf#Scene0" { .. }`. Loading the file gives the result
//! - the gltf stays usable alone, Rust attaches files to it when loading:
//!   `corn_scenes.attach("level.gltf#Scene0", &["level.corn", "props.corn"])` gives a new scene: the gltf
//!   with the files' items applied to its root, their `spawn`s as children
//! - a file attaches them: `spawn level { include "level.gltf#Scene0"; include "level.corn"; }`
//!
//! `include` copies at load time, so outer rules reach into what's included. A child that keeps its own
//! rules is a component instead: `WorldAssetRoot: "x.corn";` spawns it when the parent spawns.
//!
//! - [`doc`]: the syntax and the parsed form; [`names`]: component names and crate precedence
//! - [`value`]: values, how they patch reflected data, and parser hooks for types
//! - [`build`]: building the scene: includes, rules, spawning once, scripts
//! - [`js_scan`]: finding the end of inline scripts

pub mod build;
pub mod doc;
pub mod js_scan;
pub mod lexer;
pub mod names;
#[cfg(feature = "scripting")]
mod scripting;
pub mod value;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::reflect::{TypeRegistry, TypeRegistryArc};
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use corn_assets::ReflectInlineAsset;

pub use build::{Classes, Included, SceneFileRoot, SceneListener, SceneListeners, Spawned};
pub use value::{ParseCx, ParseValue, ReflectParseValue, RegisterValueParser, Value};

/// parses a scene file; `precedence` may be empty (the default order) or partial (completed by default)
pub fn parse_doc(src: &str, path: &str, registry: &TypeRegistry, precedence: &[String]) -> Result<doc::Doc, String> {
    let index = names::TypeIndex::new(registry);
    let precedence = names::parse_precedence(&precedence.join("\n"), index.crates());
    let resolver = names::Resolver::new(&index, &precedence);
    match doc::parse(src, path, resolver) {
        Ok((doc, warnings)) => {
            for w in warnings {
                warn!("{w}");
            }
            Ok(doc)
        }
        Err(errors) => Err(errors.join("\n")),
    }
}

/// the crate precedence for bare component names, shared with the loader
#[derive(Resource, Clone, Default)]
pub struct ScenePrecedence(pub Arc<RwLock<Vec<String>>>);

/// the label of a scene file's script module
pub const SCRIPT_LABEL: &str = "$script";
/// the label of a scene file's [`CornFile`]
pub const DOC_LABEL: &str = "$doc";

// CLAUDE
/// a scene file in parsed form, labeled [`DOC_LABEL`]: what including the file pastes
#[derive(Asset, TypePath)]
pub struct CornFile {
    pub(crate) source: Arc<build::Source>,
}

// CLAUDE
/// makes scenes from a scene and `.corn` files attached to it, see [`CornScenes::attach`]
#[derive(Resource, Clone)]
pub struct CornScenes {
    assets: AssetServer,
    requests: async_channel::Sender<build::Request>,
    /// what's been made, to give the same scene for the same arguments while it's alive
    made: Arc<Mutex<HashMap<(String, Vec<String>), AssetId<WorldAsset>>>>,
}

impl CornScenes {
    // CLAUDE
    /// a new scene: `scene` (gltf or `.corn`) with `files` applied to it in order, built as a file that
    /// only includes them would be. The scene and files stay usable on their own. The result has no
    /// asset path: for other files to use it, they include the same paths
    pub fn attach(&self, scene: &str, files: &[&str]) -> Handle<WorldAsset> {
        let paths: Vec<String> = std::iter::once(scene).chain(files.iter().copied()).map(str::to_string).collect();
        let key = (scene.to_string(), paths[1..].to_vec());
        let mut made = self.made.lock().unwrap();
        if let Some(handle) = made.get(&key).and_then(|&id| self.assets.get_id_handle(id)) {
            return handle;
        }
        let (assets, requests) = (self.assets.clone(), self.requests.clone());
        let handle = self.assets.add_async(async move {
            let fail = SceneFileError;
            let body = paths.iter().map(|path| doc::Item::Include { path: path.clone(), child: false, body: None, loc: default() }).collect();
            let path = paths.join(" + ");
            let doc = doc::Doc { path: path.clone(), body: Arc::new(body), scripts: vec![], inline: vec![] };
            let includes = paths
                .iter()
                .map(|p| {
                    let include = match doc::split_include(p).0.ends_with(".corn") {
                        true => build::Include::File(assets.load(format!("{}#{DOC_LABEL}", doc::split_include(p).0))),
                        false => build::Include::Scene(assets.load(p.clone())),
                    };
                    (p.clone(), include)
                })
                .collect();
            wait_for_includes(&assets, &path, &includes).await.map_err(fail)?;
            let source = build::Source { doc: Arc::new(doc), module: format!("{path}#{SCRIPT_LABEL}"), includes, named: default() };
            let built = request_build(&requests, Arc::new(source)).await.map_err(fail)?;
            Ok::<_, SceneFileError>(WorldAsset::new(built.world))
        });
        made.insert(key, handle.id());
        handle
    }
}

// CLAUDE
async fn wait_for_includes(assets: &AssetServer, path: &str, includes: &HashMap<String, build::Include>) -> Result<(), String> {
    for (include, handle) in includes {
        let waited = match handle {
            build::Include::Scene(handle) => assets.wait_for_asset(handle).await,
            build::Include::File(handle) => assets.wait_for_asset(handle).await,
        };
        waited.map_err(|e| format!("{path}: include \"{include}\": {e}"))?;
    }
    Ok(())
}

// CLAUDE
async fn request_build(requests: &async_channel::Sender<build::Request>, source: Arc<build::Source>) -> Result<build::Built, String> {
    let (reply, built) = async_channel::bounded(1);
    requests.send(build::Request { source, reply }).await.map_err(|e| e.to_string())?;
    built.recv().await.map_err(|e| e.to_string())?
}

#[derive(TypePath)]
struct SceneFileLoader {
    registry: TypeRegistryArc,
    precedence: ScenePrecedence,
    assets: AssetServer,
    requests: async_channel::Sender<build::Request>,
    /// scene files waiting for the scene files they include, to catch cycles
    waiting: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl FromWorld for SceneFileLoader {
    fn from_world(world: &mut World) -> Self {
        SceneFileLoader {
            registry: world.resource::<AppTypeRegistry>().0.clone(),
            precedence: world.get_resource_or_init::<ScenePrecedence>().clone(),
            assets: world.resource::<AssetServer>().clone(),
            requests: world.resource::<CornScenes>().requests.clone(),
            waiting: Arc::default(),
        }
    }
}

#[derive(Debug)]
pub struct SceneFileError(String);

impl std::fmt::Display for SceneFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SceneFileError {}

// CLAUDE
/// records that `file` waits for `includes` until dropped; an error if that closes a cycle
struct Waiting {
    waiting: Arc<Mutex<HashMap<String, Vec<String>>>>,
    file: String,
}

impl Waiting {
    fn start(waiting: &Arc<Mutex<HashMap<String, Vec<String>>>>, file: &str, includes: Vec<String>) -> Result<Self, String> {
        let mut graph = waiting.lock().unwrap();
        // is `file` reachable from what it includes?
        let mut stack: Vec<(String, Vec<String>)> = includes.iter().map(|i| (i.clone(), vec![file.to_string(), i.clone()])).collect();
        let mut seen = HashSet::new();
        while let Some((at, chain)) = stack.pop() {
            if at == file {
                return Err(format!("includes itself: {}", chain.join(" -> ")));
            }
            if !seen.insert(at.clone()) {
                continue;
            }
            for next in graph.get(&at).into_iter().flatten() {
                let mut chain = chain.clone();
                chain.push(next.clone());
                stack.push((next.clone(), chain));
            }
        }
        graph.insert(file.to_string(), includes);
        Ok(Waiting { waiting: waiting.clone(), file: file.to_string() })
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.waiting.lock().unwrap().remove(&self.file);
    }
}

impl AssetLoader for SceneFileLoader {
    type Asset = WorldAsset;
    type Settings = ();
    type Error = SceneFileError;

    // CLAUDE
    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<WorldAsset, SceneFileError> {
        let fail = SceneFileError;
        let mut bytes = vec![];
        reader.read_to_end(&mut bytes).await.map_err(|e| fail(e.to_string()))?;
        let src = String::from_utf8(bytes).map_err(|e| fail(e.to_string()))?;
        let path = load_context.path().to_string();
        let doc = {
            let precedence = self.precedence.0.read().unwrap().clone();
            parse_doc(&src, &path, &self.registry.read(), &precedence).map_err(fail)?
        };

        // includes: in before building
        let includes: HashMap<String, build::Include> = doc
            .include_paths()
            .into_iter()
            .map(|p| {
                let file = doc::split_include(&p).0;
                let include = match file.ends_with(".corn") {
                    true => build::Include::File(load_context.load(format!("{file}#{DOC_LABEL}"))),
                    false => build::Include::Scene(load_context.load(p.clone())),
                };
                (p, include)
            })
            .collect();
        let scene_files = includes
            .keys()
            .map(|p| doc::split_include(p).0.to_string())
            .filter(|file| file.ends_with(".corn"))
            .collect();
        let waiting = Waiting::start(&self.waiting, &path, scene_files).map_err(|e| fail(format!("{path}: {e}")))?;
        wait_for_includes(&self.assets, &path, &includes).await.map_err(fail)?;
        drop(waiting);

        // labels: fragments, assets (their handles now, filled once made), the script module, the doc
        let mut labels = HashSet::from([SCRIPT_LABEL.to_string(), DOC_LABEL.to_string()]);
        for (name, _) in doc.fragments() {
            if !labels.insert(name.to_string()) {
                return Err(fail(format!("{path}: `spawn {name}` twice at the top level: each is the scene `{path}#{name}`")));
            }
        }
        let mut named = HashMap::new();
        {
            let registry = self.registry.read();
            for item in doc.body.iter() {
                let doc::Item::Asset { name, asset, type_name, .. } = item else { continue };
                if !labels.insert(name.clone()) {
                    return Err(fail(format!("{path}: `asset {name}` has the name of a top-level `spawn`")));
                }
                let inline = registry.get_type_data::<ReflectInlineAsset>(*asset).ok_or_else(|| {
                    fail(format!("{path}: asset {name}: `{type_name}` can't be made from data (register it with corn_assets::register_inline or register_recipe)"))
                })?;
                named.insert(name.clone(), inline.label_handle(load_context, name.clone()));
            }
        }

        let doc = Arc::new(doc);
        let module = format!("{path}#{SCRIPT_LABEL}");
        let source = Arc::new(build::Source { doc: doc.clone(), module, includes, named });
        let built = request_build(&self.requests, source.clone()).await.map_err(fail)?;

        {
            let registry = self.registry.read();
            for (name, asset, value) in built.assets {
                let inline = registry.get_type_data::<ReflectInlineAsset>(asset).unwrap();
                inline.add_labeled(load_context, name.clone(), value.as_ref()).map_err(|e| fail(format!("{path}: asset {name}: {e}")))?;
            }
        }
        for (name, world) in built.fragments {
            load_context.add_labeled_asset(name, WorldAsset::new(world));
        }
        #[cfg(feature = "scripting")]
        if let Some(source) = doc.module_source() {
            load_context.add_labeled_asset(SCRIPT_LABEL.to_string(), corn_scripting::ScriptModule { source });
        }
        load_context.add_labeled_asset(DOC_LABEL.to_string(), CornFile { source });
        Ok(WorldAsset::new(built.world))
    }

    fn extensions(&self) -> &[&str] {
        &["corn"]
    }
}

/// loads `.corn` files as scene assets (`WorldAsset`)
pub struct SceneFilePlugin {
    /// the crate precedence file; written with the default order when missing. None: default order
    pub precedence_file: Option<PathBuf>,
}

impl Default for SceneFilePlugin {
    fn default() -> Self {
        SceneFilePlugin { precedence_file: (!cfg!(target_family = "wasm")).then(|| "assets/scene_precedence.txt".into()) }
    }
}

impl Plugin for SceneFilePlugin {
    fn build(&self, app: &mut App) {
        let (requests, served) = async_channel::unbounded();
        let assets = app.world().resource::<AssetServer>().clone();
        app.init_resource::<ScenePrecedence>()
            .insert_resource(CornScenes { assets, requests, made: default() })
            .insert_resource(build::Requests(served))
            .init_resource::<build::ChildScenes>()
            .init_asset::<CornFile>()
            .init_asset_loader::<SceneFileLoader>()
            .register_type::<SceneFileRoot>()
            .register_type::<Classes>()
            .register_type::<Spawned>()
            .register_type::<Included>()
            .register_type::<SceneListeners>()
            // a child that keeps its own rules: `WorldAssetRoot: "x.corn";`
            .register_type::<WorldAssetRoot>()
            // every scene file has a hierarchy and names
            .register_type::<ChildOf>()
            .register_type::<Children>()
            .register_type::<Name>()
            .add_systems(Update, build::serve);
        value::register_builtin(app);
        corn_assets::register::<WorldAsset>(app);
        #[cfg(feature = "scripting")]
        scripting::plugin(app);
        #[cfg(feature = "mesh")]
        value::register_mesh(app);
    }

    // CLAUDE
    /// reads (or writes) the precedence file once every type is registered
    fn finish(&self, app: &mut App) {
        let Some(file) = &self.precedence_file else { return };
        let index = names::TypeIndex::new(&app.world().resource::<AppTypeRegistry>().read());
        let list = match std::fs::read_to_string(file) {
            Ok(text) => names::parse_precedence(&text, index.crates()),
            Err(_) => {
                let list = names::default_precedence(index.crates());
                match std::fs::write(file, names::precedence_file_text(&list)) {
                    Ok(()) => info!("wrote the default crate precedence to {}", file.display()),
                    Err(e) => warn!("can't write {}: {e}", file.display()),
                }
                list
            }
        };
        *app.world().resource::<ScenePrecedence>().0.write().unwrap() = list;
    }
}
