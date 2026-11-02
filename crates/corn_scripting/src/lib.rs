//! CLAUDE (claude-opus-5-5): scripting for bevy. JavaScript: V8 on native, the browser's engine on web.
//!
//! **Modules.** Script code lives in modules: a [`ScriptModule`] asset named by its
//! asset path, e.g. `scripts/death.js`, a labeled sub-asset such as `scenes/menu.css#script`, or one
//! added in code with [`ScriptModules::insert`]. A module is evaluated once, so module-level state is
//! shared by every call into it (its top-level variables). Modules export functions taking
//! `(target, args, event)`: `exports.name = function* (target, args, event) { ... }` (or a plain function).
//!
//! **Running functions.**
//! - On events: a [`ScriptListener`] entity names an event type, module and function. With [`ListenerOf`]
//!   it hears the event on that entity (and despawns with it), without, on every entity. Events are rust
//!   types registered with [`ScriptEventsExt::register_script_event`] (or `register_script_entity_event`
//!   for events with a target). For the component lifecycle events
//!   `OnAdd`, `OnInsert` and `OnRemove`, it also names the component. Listener calls are queued and run
//!   by the script driver once per frame, so every rust observer has seen the event first.
//! - From rust: the [`RunScript`] command calls the function as it's applied, even when that happens
//!   inside another script (a host call that flushes commands). Before its module has loaded, it queues.
//!
//! A function that is a generator runs over time: `yield seconds` sleeps (bare `yield` = one frame) and
//! evaluates to the time actually slept. The driver steps it until it finishes or its owner (the
//! listener, or the RunScript target) is despawned.
//!
//! **Host api**. values cross as JSON through bevy_reflect:
//! - `insert(entity, "Type", {...})` patches the component, or inserts it (Default + patch, else the full value)
//! - `get(entity, "Type")` the component as reflect-serialized data (a math value for math types), or null
//! - `get_resource("Type")` likewise for a resource
//! - `remove(entity, "Type")`, `despawn(entity)`
//! - `find("Type")` the entities that have the component
//! - `spawn({ "Type": {...}, ... })` a new entity with these components, returns it
//! - "Type" may also name a bundle the app registered for scripts ([`ScriptBundlesExt`]), and a `Handle`
//!   field may take an object that makes a new asset (see [`values`])
//! - `component(entity, "Type")` a live reference: reading a field gives a plain value (number, bool,
//!   string, entity) or another live reference; writing a field writes the component. see [`live`]. `ref.field`, `ref[0]`.
//! - `resource("Type")` likewise for a resource, null if it doesn't exist
//! - `asset(handle, "Type")` likewise for an asset, by what a `Handle` field reads as (its path, or an id
//!   if it has none), null if not loaded. the asset type must be registered with `register_asset_reflect`
//! - `trigger(entity | null, "Event", {...})` triggers a registered event. rust observers run right away,
//!   script listeners are queued.
//! - `mesh()` and the shapes `Cuboid(x, y, z)`, `Sphere(r)`, `Cylinder(r, h)`, `Capsule3d(r, len)`,
//!   `Cone(r, h)`, `Torus(inner, outer)`, `Plane3d(w, d)`: a mesh built in rust from a recipe
//!   (corn_assets::mesh), as the value of a `Handle<Mesh>` field: `insert(e, "Mesh3d", Cuboid(1, 2, 1))`,
//!   `mesh().add(Cylinder(0.1, 3), { translate: [0, 1.5, 0] }).add(Sphere(0.3), { translate: [0, 3, 0] })`
//!
//! "Type" is a full type path, or a short one if unambiguous. `Handle` fields are asset paths (see
//! [`values`]), entities are their id strings ([`entity_key`]).
//!
//! **Scene files** (corn_scene) build their scene at load time, on the main thread: their module (one
//! labeled `ScriptModule` per file) loads with [`load_scene_module`] and their `${ .. }` values run with
//! [`eval_in_scene`], against the scene's own world. There the host api acts on the scene, and
//! `world.get`, `world.get_resource` and `world.find` read the app's world. Their `on` handlers become
//! `ScriptListener` + `ListenerOf` once the scene spawns, calling into the same module instance.

use std::{cell::Cell, collections::HashMap, ptr, rc::Rc};

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext, LoadState},
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    reflect::{
        serde::{TypedReflectDeserializer, TypedReflectSerializer},
        ReflectMut, TypeRegistration, TypeRegistry,
    },
};
use serde::de::DeserializeSeed;

pub use events::ScriptEventsExt;
use values::ScriptValues;

mod events;
mod live;
mod math;
pub mod values;
#[cfg(test)]
mod bench;
#[cfg(not(target_family = "wasm"))]
#[path = "js_native.rs"]
mod js;
#[cfg(target_family = "wasm")]
#[path = "js_web.rs"]
mod js;

/// js half of the runtime, shared by the native and web engines
const GLUE: &str = include_str!("glue.js");

/// scripts running scripts (through commands) deeper than this is an error, not a stack overflow
const MAX_DEPTH: u32 = 64;

pub fn plugin(app: &mut App) {
    app.init_asset::<ScriptModule>()
        .init_asset_loader::<ScriptModuleLoader>()
        .register_type::<ScriptListener>()
        .register_type::<ListenerOf>()
        .register_type::<ScriptListeners>()
        .init_resource::<ScriptModules>()
        .init_resource::<ScriptQueue>()
        .init_resource::<Threads>()
        .init_resource::<events::ScriptEventRegistry>()
        .insert_non_send(ScriptEngine(Rc::new(js::Js::new().expect("failed to start the js engine"))))
        .add_systems(PostUpdate, drive_scripts.before(TransformSystems::Propagate));
    events::register_lifecycle::<bevy::ecs::lifecycle::AddEvent>(app, "OnAdd");
    events::register_lifecycle::<bevy::ecs::lifecycle::InsertEvent>(app, "OnInsert");
    events::register_lifecycle::<bevy::ecs::lifecycle::RemoveEvent>(app, "OnRemove");
}

/// a resource's entity (resources are components on their own entities)
pub(crate) fn resource_entity(world: &World, type_id: std::any::TypeId) -> Option<Entity> {
    let id = world.components().get_id(type_id)?;
    world.resource_entities().get(id)
}

pub fn entity_key(entity: Entity) -> String {
    entity.to_bits().to_string()
}

#[derive(Debug, Asset, TypePath)]
pub struct ScriptModule {
    pub source: String,
}

#[derive(Default)]
#[derive(TypePath)]
struct ScriptModuleLoader;

impl AssetLoader for ScriptModuleLoader {
    type Asset = ScriptModule;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(&self, reader: &mut dyn Reader, _: &(), _: &mut LoadContext<'_>) -> Result<ScriptModule, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let source = String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(ScriptModule { source })
    }

    fn extensions(&self) -> &[&str] {
        &["js"]
    }
}

// CLAUDE
/// modules by name, and whether they're loaded into their engine
#[derive(Default, Resource)]
pub struct ScriptModules(HashMap<String, Module>);

enum Module {
    Loading(Handle<ScriptModule>),
    Ready,
    Failed,
}

impl ScriptModules {
    /// add a module that isn't an asset file, e.g. one built in code
    pub fn insert(&mut self, assets: &mut Assets<ScriptModule>, name: impl Into<String>, module: ScriptModule) {
        self.0.insert(name.into(), Module::Loading(assets.add(module)));
    }

    /// start loading the module's asset, if this is the first we've heard of it
    fn request(&mut self, name: &str, assets: &AssetServer) {
        if !self.0.contains_key(name) {
            self.0.insert(name.to_string(), Module::Loading(assets.load(name.to_string())));
        }
    }
}

/// whether the module is evaluated: false while its asset loads
fn ready(world: &mut World, engine: &js::Js, name: &str) -> Result<bool, String> {
    let handle = match world.resource::<ScriptModules>().0.get(name) {
        Some(Module::Ready) => return Ok(true),
        Some(Module::Failed) => return Err("the module failed to load".into()),
        Some(Module::Loading(handle)) => handle.clone(),
        None => {
            let assets = world.resource::<AssetServer>().clone();
            world.resource_mut::<ScriptModules>().request(name, &assets);
            return Ok(false);
        }
    };
    let Some(module) = world.resource::<Assets<ScriptModule>>().get(&handle) else {
        return match world.resource::<AssetServer>().load_state(&handle) {
            LoadState::Failed(e) => {
                world.resource_mut::<ScriptModules>().0.insert(name.to_string(), Module::Failed);
                Err(e.to_string())
            }
            _ => Ok(false),
        };
    };
    let source = module.source.clone();
    let result = with_world_scope(world, || engine.load(name, &source));
    let state = if result.is_ok() { Module::Ready } else { Module::Failed };
    world.resource_mut::<ScriptModules>().0.insert(name.to_string(), state);
    result.map(|()| true)
}

// CLAUDE
/// Calls `module`'s `function` when its `event` is triggered on the [`ListenerOf`] entity (or any
/// entity, without one). Usually spawned by scene data.
#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component, Default)]
#[component(on_insert = ScriptListener::on_insert)]
pub struct ScriptListener {
    /// a registered event type, full or short type path
    pub event: String,
    pub module: String,
    pub function: String,
    /// JSON passed to the function. empty = `{}`
    pub args: String,
    /// lifecycle events only: the component added/inserted/removed
    pub component: String,
}

impl ScriptListener {
    pub fn new(event: impl Into<String>, module: impl Into<String>, function: impl Into<String>) -> Self {
        Self { event: event.into(), module: module.into(), function: function.into(), ..default() }
    }

    pub fn with_args(mut self, args: serde_json::Value) -> Self {
        self.args = args.to_string();
        self
    }

    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = component.into();
        self
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let listener = world.get::<ScriptListener>(entity).unwrap().clone();
        let assets = world.resource::<AssetServer>().clone();
        world.resource_mut::<ScriptModules>().request(&listener.module, &assets);
        // registering a component and spawning a bridge need the whole world
        world.commands().queue(move |world: &mut World| {
            let event = events::resolve_listener(world, &listener);
            match (event, world.get_entity_mut(entity)) {
                (Ok(event), Ok(mut listener)) => {
                    listener.insert(event);
                }
                (Err(e), _) => error!(%entity, "script listener: {e}"),
                (_, Err(_)) => {}
            }
        });
    }
}

/// the entity a [`ScriptListener`] listens on
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship(relationship_target = ScriptListeners)]
pub struct ListenerOf(pub Entity);

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship_target(relationship = ListenerOf, linked_spawn)]
pub struct ScriptListeners(Vec<Entity>);

// CLAUDE
/// Calls `module`'s `function` with `target`, as the command is applied.
pub struct RunScript {
    pub module: String,
    pub function: String,
    pub target: Entity,
    pub args: serde_json::Value,
}

impl RunScript {
    pub fn new(module: impl Into<String>, function: impl Into<String>, target: Entity) -> Self {
        Self { module: module.into(), function: function.into(), target, args: serde_json::json!({}) }
    }

    pub fn with_args(mut self, args: serde_json::Value) -> Self {
        self.args = args;
        self
    }
}

impl Command for RunScript {
    type Out = ();

    fn apply(self, world: &mut World) {
        let engine = world.non_send::<ScriptEngine>().0.clone();
        let call = Call {
            module: self.module,
            function: self.function,
            owner: Some(self.target),
            target: Some(self.target),
            args: self.args.to_string(),
            event: "null".into(),
        };
        if let Some(call) = start(world, &engine, call) {
            world.resource_mut::<ScriptQueue>().0.push(call);
        }
    }
}

/// a function call for the driver
struct Call {
    module: String,
    function: String,
    /// its threads stop when this is despawned
    owner: Option<Entity>,
    target: Option<Entity>,
    args: String,
    event: String,
}

#[derive(Default, Resource)]
struct ScriptQueue(Vec<Call>);

/// a running generator
struct Thread {
    id: u32,
    owner: Option<Entity>,
    /// `module#function`, for errors
    name: String,
    sleep: f64,
    slept: f64,
}

#[derive(Default, Resource)]
struct Threads {
    next: u32,
    running: Vec<Thread>,
}

// CLAUDE
/// The js runtime (`js::Js`, per platform). Methods take `&self`: a script can run while another is mid host call.
trait Engine {
    /// evaluate a module, replacing any previous one with that name
    fn load(&self, module: &str, source: &str) -> Result<(), String>;
    /// call an exported function. true if it returned a generator, which is then stepped as `thread`.
    /// `args` and `event` are JSON
    fn call(&self, module: &str, function: &str, thread: u32, target: Option<&str>, args: &str, event: &str)
        -> Result<bool, String>;
    /// resume a thread. seconds to sleep, or None when finished
    fn step(&self, thread: u32, slept: f64) -> Result<Option<f64>, String>;
    fn stop(&self, thread: u32);
    /// call an exported function as `(target, root)` for its value: JSON, empty for `undefined`
    fn evaluate(&self, module: &str, function: &str, target: &str, root: &str) -> Result<String, String>;
}

/// not Send: the engine must stay on the main thread. an Rc so a script can run while the world is
/// borrowed by another
struct ScriptEngine(Rc<js::Js>);

/// call a function, running it to its first yield. gives the call back if its module isn't loaded yet
fn start(world: &mut World, engine: &js::Js, call: Call) -> Option<Call> {
    let name = format!("{}#{}", call.module, call.function);
    match ready(world, engine, &call.module) {
        Ok(true) => {}
        Ok(false) => return Some(call),
        Err(e) => {
            error!(script = name, "{e}");
            return None;
        }
    }
    let id = {
        let mut threads = world.resource_mut::<Threads>();
        threads.next = threads.next.wrapping_add(1);
        threads.next
    };
    let target = call.target.map(entity_key);
    let args = if call.args.trim().is_empty() { "{}" } else { &call.args };
    let result = with_world_scope(world, || {
        match engine.call(&call.module, &call.function, id, target.as_deref(), args, &call.event)? {
            true => engine.step(id, 0.0),
            false => Ok(None),
        }
    });
    match result {
        Ok(Some(sleep)) => {
            let thread = Thread { id, owner: call.owner, name, sleep, slept: 0.0 };
            world.resource_mut::<Threads>().running.push(thread);
        }
        Ok(None) => {}
        Err(e) => error!(script = name, "{e}"),
    }
    None
}

// CLAUDE
/// The script driver: resumes sleeping threads, then runs the queued calls. Calls queued while it runs
/// wait for the next frame.
fn drive_scripts(world: &mut World) {
    let engine = world.non_send::<ScriptEngine>().0.clone();
    let dt = world.resource::<Time>().delta_secs_f64();

    let mut due = Vec::new();
    let mut orphaned = Vec::new();
    world.resource_scope(|world, mut threads: Mut<Threads>| {
        threads.running.retain_mut(|thread| {
            if thread.owner.is_some_and(|owner| world.get_entity(owner).is_err()) {
                orphaned.push(thread.id);
                return false;
            }
            thread.slept += dt;
            if thread.slept >= thread.sleep {
                due.push((thread.id, thread.slept));
            }
            true
        });
    });
    for id in orphaned {
        engine.stop(id);
    }
    for (id, slept) in due {
        // an earlier thread may have despawned this one's owner
        let owner = world.resource::<Threads>().running.iter().find(|t| t.id == id).and_then(|t| t.owner);
        if owner.is_some_and(|owner| world.get_entity(owner).is_err()) {
            engine.stop(id);
            continue;
        }
        let result = with_world_scope(world, || engine.step(id, slept));
        resumed(world, id, result);
    }

    let calls = std::mem::take(&mut world.resource_mut::<ScriptQueue>().0);
    let waiting: Vec<Call> = calls.into_iter().filter_map(|call| start(world, &engine, call)).collect();
    world.resource_mut::<ScriptQueue>().0.splice(0..0, waiting);
}

/// record a step: keep the thread sleeping, or drop it when finished or failed
fn resumed(world: &mut World, id: u32, result: Result<Option<f64>, String>) {
    let mut threads = world.resource_mut::<Threads>();
    let Some(index) = threads.running.iter().position(|t| t.id == id) else {
        return;
    };
    match result {
        Ok(Some(sleep)) => {
            let thread = &mut threads.running[index];
            thread.sleep = sleep;
            thread.slept = 0.0;
        }
        Ok(None) => {
            threads.running.remove(index);
        }
        Err(e) => {
            let thread = threads.running.remove(index);
            error!(script = thread.name, "{e}");
        }
    }
}

thread_local! {
    static WORLD: Cell<*mut World> = const { Cell::new(ptr::null_mut()) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// while a scene builds: the app world, which `world.*` reads
    static APP_WORLD: Cell<*mut World> = const { Cell::new(ptr::null_mut()) };
    /// the scene world while `world.*` has swapped it out
    static SWAPPED: Cell<*mut World> = const { Cell::new(ptr::null_mut()) };
    /// what `evaluate` gave, as JSON
    static RESULT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

// CLAUDE
/// Scene files run module code while their scene builds (at load, on the main thread). This loads a
/// module against `scene`, that scene's world: the host api acts on the scene, `world.get`,
/// `world.get_resource` and `world.find` read `app`, and triggering events fails. `scene` needs no
/// resources: the type registry and asset server come from `app`. The module stays loaded for later
/// calls, such as the scene's event handlers once it is spawned.
pub fn load_scene_module(app: &mut World, scene: &mut World, name: &str, source: &str) -> Result<(), String> {
    let engine = app.non_send::<ScriptEngine>().0.clone();
    in_scene(app, scene, || engine.load(name, source))?;
    app.resource_mut::<ScriptModules>().0.insert(name.to_string(), Module::Ready);
    Ok(())
}

/// calls `function(target, root)` of a module loaded with [`load_scene_module`], against `scene`, and
/// gives its value as JSON
pub fn eval_in_scene(app: &mut World, scene: &mut World, module: &str, function: &str, target: Entity, root: Entity) -> Result<serde_json::Value, String> {
    let engine = app.non_send::<ScriptEngine>().0.clone();
    let json = in_scene(app, scene, || engine.evaluate(module, function, &entity_key(target), &entity_key(root)))?;
    if json.is_empty() {
        return Err("no value (the last statement is the value)".into());
    }
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

fn in_scene<R>(app: &mut World, scene: &mut World, f: impl FnOnce() -> Result<R, String>) -> Result<R, String> {
    struct Restore(*mut World);
    impl Drop for Restore {
        fn drop(&mut self) {
            APP_WORLD.set(self.0);
        }
    }
    let _restore = Restore(APP_WORLD.replace(app));
    with_world_scope(scene, f)
}

/// the type registry: the world's, or while a scene builds (its world has no resources) the app's
fn type_registry(world: &World) -> AppTypeRegistry {
    world.get_resource::<AppTypeRegistry>().cloned().unwrap_or_else(|| {
        let app = APP_WORLD.get();
        assert!(!app.is_null(), "scripts need an AppTypeRegistry");
        // SAFETY: only non-null inside `in_scene`, which holds the `&mut World` and doesn't use it while
        // the script runs
        unsafe { &*app }.resource::<AppTypeRegistry>().clone()
    })
}

/// the asset server: the world's, or while a scene builds the app's
fn asset_server(world: &World) -> Option<AssetServer> {
    world.get_resource::<AssetServer>().cloned().or_else(|| {
        let app = APP_WORLD.get();
        // SAFETY: as in `type_registry`
        (!app.is_null()).then(|| unsafe { &*app }.get_resource::<AssetServer>().cloned()).flatten()
    })
}

/// host: `world.*` swaps the app world in while a scene builds. elsewhere the app world is the world
fn host_use_app(on: bool) -> Result<(), String> {
    let app = APP_WORLD.get();
    if app.is_null() {
        return Ok(());
    }
    if on {
        SWAPPED.set(WORLD.replace(app));
    } else {
        WORLD.set(SWAPPED.replace(ptr::null_mut()));
    }
    live::clear_cache();
    Ok(())
}

fn host_result(json: String) {
    RESULT.set(Some(json));
}

/// make `world` reachable from host functions while `f` runs. Scopes nest: a host call can flush a
/// command that runs another script, with a `&mut World` reborrowed from the host call's.
/// Live refs resolve afresh in each scope.
fn with_world_scope<R>(world: &mut World, f: impl FnOnce() -> Result<R, String>) -> Result<R, String> {
    if DEPTH.get() >= MAX_DEPTH {
        return Err(format!("scripts nested more than {MAX_DEPTH} deep"));
    }
    struct Restore(*mut World);
    impl Drop for Restore {
        fn drop(&mut self) {
            WORLD.set(self.0);
            DEPTH.set(DEPTH.get() - 1);
            live::clear_cache();
        }
    }
    live::clear_cache();
    DEPTH.set(DEPTH.get() + 1);
    let _restore = Restore(WORLD.replace(world));
    f()
}

fn with_host_world<R>(f: impl FnOnce(&mut World) -> Result<R, String>) -> Result<R, String> {
    let world = WORLD.get();
    if world.is_null() {
        return Err("host functions can only be called while a script is running".into());
    }
    // SAFETY: only non-null inside `with_world_scope`, which holds the `&mut World` and doesn't use it
    // while the script runs. a nested scope's world is reborrowed from this one's host call, and is
    // restored before that call continues.
    f(unsafe { &mut *world })
}

// CLAUDE
fn host_trigger(target: Option<&str>, event: &str, json: &str) -> Result<(), String> {
    if !APP_WORLD.get().is_null() {
        return Err("events can't be triggered while a scene loads".into());
    }
    let data: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let target = target.map(parse_entity).transpose()?;
    with_host_world(|world| events::trigger(world, target, event, &data))
}

// CLAUDE
fn host_insert(entity: &str, type_name: &str, json: &str) -> Result<(), String> {
    let patch: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let entity = parse_entity(entity)?;
    with_host_world(|world| insert_json(world, entity, type_name, &patch))
}

// CLAUDE
/// replace a math component (`Transform`...) with a math value
fn host_insert_math(entity: &str, type_name: &str, value: math::Math) -> Result<(), String> {
    let entity = parse_entity(entity)?;
    with_host_world(|world| {
        live::clear_cache();
        let registry = type_registry(world);
        let registry = registry.read();
        let (registration, component) = lookup(&registry, type_name)?;
        if math::Kind::of(registration.type_id()) != Some(value.kind()) {
            return Err(format!("can't insert a {} as `{type_name}`", value.kind().name()));
        }
        let mut entity = world.get_entity_mut(entity).map_err(|e| e.to_string())?;
        component.insert(&mut entity, value.as_reflect(), &registry);
        Ok(())
    })
}

/// what `get` gives
enum Got {
    Json(String),
    /// math types are the engine's own math values
    Math(math::Math),
}

// CLAUDE
fn host_get(entity: &str, type_name: &str) -> Result<Got, String> {
    with_host_world(|world| {
        let entity = parse_entity(entity)?;
        let registry = type_registry(world);
        let registry = registry.read();
        let (_, component) = lookup(&registry, type_name)?;
        let entity = world.get_entity(entity).map_err(|e| e.to_string())?;
        match component.reflect(entity) {
            Some(value) => copy(value.as_partial_reflect(), &registry),
            None => Ok(Got::Json("null".into())),
        }
    })
}

// CLAUDE
/// a resource, as `get` gives a component. null if it doesn't exist
fn host_get_resource(type_name: &str) -> Result<Got, String> {
    with_host_world(|world| {
        let registry = type_registry(world);
        let registry = registry.read();
        let registration = registration(&registry, type_name)?;
        if !registration.contains::<ReflectResource>() {
            return Err(format!("`{type_name}` is not a reflected resource"));
        }
        let component = registration.data::<ReflectComponent>().ok_or("a reflected resource is a reflected component")?;
        let value = resource_entity(world, registration.type_id()).and_then(|e| component.reflect(world.entity(e)));
        match value {
            Some(value) => copy(value.as_partial_reflect(), &registry),
            None => Ok(Got::Json("null".into())),
        }
    })
}

/// what `get` gives for a value: a math value, or JSON
fn copy(value: &dyn PartialReflect, registry: &TypeRegistry) -> Result<Got, String> {
    if let Some(math) = math::Math::of(value) {
        return Ok(Got::Math(math));
    }
    let processor = ScriptValues(None);
    serde_json::to_string(&TypedReflectSerializer::with_processor(value, registry, &processor))
        .map(Got::Json)
        .map_err(|e| e.to_string())
}

// CLAUDE
fn host_remove(entity: &str, type_name: &str) -> Result<(), String> {
    with_host_world(|world| {
        let entity = parse_entity(entity)?;
        let registry = type_registry(world);
        let registry = registry.read();
        let (_, component) = lookup(&registry, type_name)?;
        live::clear_cache();
        component.remove(&mut world.get_entity_mut(entity).map_err(|e| e.to_string())?);
        Ok(())
    })
}

// CLAUDE
fn host_spawn(json: &str) -> Result<String, String> {
    let components: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(json).map_err(|e| format!("spawn takes an object of components: {e}"))?;
    with_host_world(|world| {
        let entity = world.spawn_empty().id();
        for (type_name, value) in &components {
            insert_json(world, entity, type_name, value)?;
        }
        Ok(entity_key(entity))
    })
}

// CLAUDE
/// the entities with a component, as a JSON array
fn host_find(type_name: &str) -> Result<String, String> {
    with_host_world(|world| {
        let registry = type_registry(world);
        let type_id = lookup(&registry.read(), type_name)?.0.type_id();
        let Some(id) = world.components().get_id(type_id) else {
            return Ok("[]".into());
        };
        let mut query = QueryBuilder::<Entity>::new(world).with_id(id).build();
        let entities: Vec<String> = query.iter(world).map(entity_key).collect();
        serde_json::to_string(&entities).map_err(|e| e.to_string())
    })
}

/// an entity that's already gone is fine: scripts hold entities across yields
fn host_despawn(entity: &str) -> Result<(), String> {
    let entity = parse_entity(entity)?;
    with_host_world(|world| {
        live::clear_cache();
        if let Ok(entity) = world.get_entity_mut(entity) {
            entity.despawn();
        }
        Ok(())
    })
}

/// insert can move the entity (and run hooks), so it also forgets live ref pointers
fn insert_json(world: &mut World, entity: Entity, type_name: &str, patch: &serde_json::Value) -> Result<(), String> {
    live::clear_cache();
    if let Some(bundle) = world.get_resource::<ScriptBundles>().and_then(|b| b.0.get(type_name).copied()) {
        bundle(&mut world.get_entity_mut(entity).map_err(|e| e.to_string())?);
        return Ok(());
    }
    let registry = type_registry(world);
    let registry = registry.read();
    let assets = asset_server(world);
    let assets = assets.as_ref();
    let (registration, component) = lookup(&registry, type_name)?;
    let mutable = is_mutable(world, component);
    let mut entity = world.get_entity_mut(entity).map_err(|e| e.to_string())?;

    if mutable {
        if let Some(mut existing) = component.reflect_mut(&mut entity) {
            return apply_json(existing.as_partial_reflect_mut(), patch, &registry, assets);
        }
    }

    // immutable components (e.g. relationships) are replaced by a patched copy
    let value = match component.reflect(&entity).filter(|_| !mutable) {
        Some(existing) => {
            let mut value = existing.to_dynamic().map_err(|e| e.to_string())?;
            apply_json(value.as_mut(), patch, &registry, assets)?;
            value
        }
        None => value_from_json(registration, &registry, assets, patch)?,
    };
    component.insert(&mut entity, value.as_ref(), &registry);
    Ok(())
}

/// a value of a registered type from script data: Default patched with it, else the data deserialized whole
pub fn value_from_json(
    registration: &TypeRegistration,
    registry: &TypeRegistry,
    assets: Option<&AssetServer>,
    json: &serde_json::Value,
) -> Result<Box<dyn PartialReflect>, String> {
    match registration.data::<ReflectDefault>() {
        Some(default) => {
            let mut value = default.default().into_partial_reflect();
            apply_json(value.as_mut(), json, registry, assets)?;
            Ok(value)
        }
        None => deserialize(registration, registry, assets, json),
    }
}

// CLAUDE
/// bundles scripts insert by name, as if they were components: `insert(entity, "Name::bundle")`, or a
/// key in `spawn({...})`. for what can't be data, like a controller with its input actions
#[derive(Default, Resource)]
pub struct ScriptBundles(HashMap<String, fn(&mut EntityWorldMut)>);

pub trait ScriptBundlesExt {
    fn register_script_bundle(&mut self, name: impl Into<String>, bundle: fn(&mut EntityWorldMut)) -> &mut Self;
}

impl ScriptBundlesExt for App {
    fn register_script_bundle(&mut self, name: impl Into<String>, bundle: fn(&mut EntityWorldMut)) -> &mut Self {
        self.world_mut().get_resource_or_init::<ScriptBundles>().0.insert(name.into(), bundle);
        self
    }
}

/// immutable components can't be changed in place, only replaced
fn is_mutable(world: &mut World, component: &ReflectComponent) -> bool {
    // registering: a component the world hasn't seen yet has no info
    let id = component.register_component(world);
    world.components().get_info(id).is_some_and(|info| info.mutable())
}

fn parse_entity(entity: &str) -> Result<Entity, String> {
    entity
        .parse()
        .ok()
        .and_then(|bits| Entity::try_from_bits(bits))
        .ok_or_else(|| format!("not an entity: `{entity}`"))
}

fn registration<'a>(registry: &'a TypeRegistry, name: &str) -> Result<&'a TypeRegistration, String> {
    registry
        .get_with_type_path(name)
        .or_else(|| registry.get_with_short_type_path(name))
        .ok_or_else(|| format!("unknown type `{name}` (not registered, or an ambiguous short name)"))
}

fn lookup<'a>(registry: &'a TypeRegistry, name: &str) -> Result<(&'a TypeRegistration, &'a ReflectComponent), String> {
    let registration = registration(registry, name)?;
    let component = registration
        .data::<ReflectComponent>()
        .ok_or_else(|| format!("`{name}` is not a reflected component"))?;
    Ok((registration, component))
}

/// json objects patch struct fields recursively, newtypes patch their field, anything else replaces the value.
/// a handle given what it reads as (null, or the id of a path-less asset) stays as it is, so `get`ed data inserts back
pub fn apply_json(
    target: &mut dyn PartialReflect,
    json: &serde_json::Value,
    registry: &TypeRegistry,
    assets: Option<&AssetServer>,
) -> Result<(), String> {
    if let serde_json::Value::Object(fields) = json {
        if let ReflectMut::Struct(target) = target.reflect_mut() {
            for (name, value) in fields {
                let field = target.field_mut(name).ok_or_else(|| format!("no field `{name}`"))?;
                apply_json(field, value, registry, assets)?;
            }
            return Ok(());
        }
    }
    // a newtype is its field's data
    if let ReflectMut::TupleStruct(newtype) = target.reflect_mut() {
        if newtype.field_len() == 1 {
            return apply_json(newtype.field_mut(0).unwrap(), json, registry, assets);
        }
    }
    if values::is_unchanged_handle(target, json, registry) {
        return Ok(());
    }
    let info = target.get_represented_type_info().ok_or("value has no type info")?;
    let registration = registry
        .get(info.type_id())
        .ok_or_else(|| format!("`{}` is not registered", info.type_path()))?;
    let value = deserialize(registration, registry, assets, json)?;
    target.try_apply(value.as_ref()).map_err(|e| e.to_string())
}

fn deserialize(
    registration: &TypeRegistration,
    registry: &TypeRegistry,
    assets: Option<&AssetServer>,
    json: &serde_json::Value,
) -> Result<Box<dyn PartialReflect>, String> {
    let mut processor = ScriptValues(assets);
    TypedReflectDeserializer::with_processor(registration, registry, &mut processor)
        .deserialize(json)
        .map_err(|e| e.to_string())
}

// CLAUDE
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, Component, Reflect)]
    #[reflect(Component, Default)]
    struct Probe {
        n: f32,
        label: String,
        source: Handle<ScriptModule>,
    }

    const JS: &str = r#"
let calls = 0;
exports.run = function* (e, args, event) {
  calls += 1;
  insert(e, "Probe", { label: `${args.label} ${event.x} ${calls}`, source: "probe.js" });
  const slept = yield 0.5;
  insert(e, "Probe", { n: get(e, "Probe").n + slept });
};
exports.spawn_child = function (e) {
  const child = spawn({ Probe: { label: get(e, "Probe").source }, ChildOf: e });
  if (get(child, "ChildOf") !== e) throw new Error("ChildOf crosses as the entity");
  insert(e, "Probe", { label: get(child, "Probe").label });
  despawn(child);
  despawn(child);
};"#;

    fn run(engine: &dyn Engine, source: &str) -> Probe {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<ScriptModule>();
        values::register::<ScriptModule>(&mut app);
        app.register_type::<Probe>().register_type::<ChildOf>();
        let world = app.world_mut();
        let entity = world.spawn_empty().id();
        let e = entity_key(entity);
        let label = |world: &World| world.get::<Probe>(entity).unwrap().label.clone();

        with_world_scope(world, || engine.load("probe", source)).unwrap();
        let call = |world: &mut World, function, thread| {
            with_world_scope(world, || engine.call("probe", function, thread, Some(&e), r#"{"label":"hi"}"#, r#"{"x":7}"#))
        };
        assert!(call(world, "run", 1).unwrap());
        assert_eq!(with_world_scope(world, || engine.step(1, 0.0)).unwrap(), Some(0.5));
        assert_eq!(label(world), "hi 7 1");
        assert_eq!(with_world_scope(world, || engine.step(1, 0.75)).unwrap(), None);
        // module state carries over to the next call
        assert!(call(world, "run", 2).unwrap());
        assert_eq!(with_world_scope(world, || engine.step(2, 0.0)).unwrap(), Some(0.5));
        assert_eq!(label(world), "hi 7 2");
        engine.stop(2);
        assert_eq!(with_world_scope(world, || engine.step(2, 1.0)).unwrap(), None, "stopped");
        // the handle crosses as its path, spawn and despawn work
        assert!(!call(world, "spawn_child", 3).unwrap());
        assert_eq!(world.query::<&Probe>().iter(world).count(), 1);
        world.entity_mut(entity).take::<Probe>().unwrap()
    }

    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn module_state_generators_and_spawn() {
        let probe = run(&js::Js::new().unwrap(), JS);
        assert_eq!((probe.n, probe.label.as_str()), (0.75, "probe.js"));
        assert_eq!(probe.source.path().unwrap().to_string(), "probe.js");
    }

    #[derive(Debug, Default, Component, Reflect)]
    #[reflect(Component, Default)]
    struct Gone;

    const LIFECYCLE_JS: &str = r#"
exports.added = (e) => insert(e, "Probe", { label: `added, ${find("Probe").length} probes` });
exports.removed = (e) => insert(e, "Gone", {});"#;

    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn lifecycle_listeners_and_find() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), plugin));
        app.register_type::<Probe>().register_type::<Gone>();
        let world = app.world_mut();
        world.resource_scope(|world, mut assets: Mut<Assets<ScriptModule>>| {
            let module = ScriptModule { source: LIFECYCLE_JS.into() };
            world.resource_mut::<ScriptModules>().insert(&mut assets, "test/lifecycle", module);
        });
        let listener = |event, function| ScriptListener::new(event, "test/lifecycle", function).with_component("Probe");
        world.spawn(listener("OnAdd", "added"));
        let [a, b] = [(); 2].map(|_| world.spawn_empty().id());
        world.spawn((listener("OnRemove", "removed"), ListenerOf(b)));
        let no_component = world.spawn(ScriptListener::new("OnAdd", "test/lifecycle", "added")).id();
        app.update();

        let world = app.world_mut();
        assert!(!world.entity(no_component).contains::<events::ListenerEvent>(), "lifecycle listeners need a component");
        world.entity_mut(a).insert(Probe::default());
        world.entity_mut(b).insert(Probe::default());
        app.update();
        let world = app.world_mut();
        for e in [a, b] {
            assert_eq!(world.get::<Probe>(e).unwrap().label, "added, 2 probes");
        }

        world.entity_mut(a).remove::<Probe>();
        world.entity_mut(b).remove::<Probe>();
        app.update();
        let world = app.world();
        assert!(!world.entity(a).contains::<Gone>(), "the OnRemove listener is b's only");
        assert!(world.entity(b).contains::<Gone>());
    }

    const MATH_JS: &str = r#"
exports.run = function (e) {
  const live = component(e, "Transform");
  live.translation.x += 1;
  const t = get(e, "Transform");
  t.translation = t.translation.add(Vec3.new(0, 2, 0));
  if (get(e, "Transform").translation.y !== 0) throw new Error("get is a copy");
  const v = Quat.from_rotation_y(Math.PI / 2).mul(Vec3.X);
  if (Math.abs(v.z + 1) > 1e-6) throw new Error(`quat * vec3: ${JSON.stringify(v)}`);
  insert(e, "Transform", t.looking_at(Vec3.ZERO, Vec3.Y));
  live.scale = Vec3.splat(2);
  spawn({ Transform: Transform.from_xyz(7, 8, 9), Probe: { n: t.translation.length() } });
};"#;

    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn math_values() {
        let engine = js::Js::new().unwrap();
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        world.resource::<AppTypeRegistry>().write().register::<Transform>();
        world.resource::<AppTypeRegistry>().write().register::<Probe>();
        let entity = world.spawn(Transform::from_xyz(1.0, 0.0, 0.0)).id();
        let e = entity_key(entity);
        with_world_scope(&mut world, || engine.load("math", MATH_JS)).unwrap();
        let result = with_world_scope(&mut world, || engine.call("math", "run", 1, Some(&e), "{}", "null"));
        assert!(!result.unwrap_or_else(|e| panic!("{e}")));

        let expected = Transform::from_xyz(2.0, 2.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y).with_scale(Vec3::splat(2.0));
        let t = world.get::<Transform>(entity).unwrap();
        assert!(t.translation.abs_diff_eq(expected.translation, 1e-5), "{t:?}");
        assert!(t.rotation.abs_diff_eq(expected.rotation, 1e-5), "{t:?}");
        assert_eq!(t.scale, expected.scale);
        let (spawned, probe) = world.query::<(&Transform, &Probe)>().single(&world).unwrap();
        assert_eq!(spawned.translation, Vec3::new(7.0, 8.0, 9.0));
        assert!((probe.n - 8f32.sqrt()).abs() < 1e-5, "{}", probe.n);
    }

    #[derive(Debug, Default, Asset, Reflect)]
    #[reflect(Default)]
    struct Paint {
        shade: f32,
        name: String,
    }

    #[derive(Debug, Default, Component, Reflect)]
    #[reflect(Component, Default)]
    struct Painted(Handle<Paint>);

    #[derive(Debug, Default, Resource, Reflect)]
    #[reflect(Resource, Default)]
    struct Weather {
        wind: f32,
    }

    #[derive(Debug, Default, Resource, Reflect)]
    #[reflect(Resource, Default)]
    struct Unset;

    const DATA_JS: &str = r#"
exports.run = function (e) {
  if (get_resource("Unset") !== null) throw new Error("a missing resource is null");
  insert(e, "Painted", { shade: get_resource("Weather").wind, name: "inline" });
  insert(e, "Probe::bundle", {});
};"#;

    /// resources, assets made from data, and bundles registered by name
    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn resources_assets_and_bundles() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), plugin))
            .init_asset::<Paint>()
            .register_type::<Painted>()
            .register_type::<Probe>()
            .register_type::<Weather>()
            .register_type::<Unset>()
            .insert_resource(Weather { wind: 2.5 })
            .register_script_bundle("Probe::bundle", |entity| {
                entity.insert((Probe { label: "bundled".into(), ..default() }, Gone));
            });
        values::register_inline::<Paint>(&mut app);
        let world = app.world_mut();
        let entity = world.spawn_empty().id();
        let e = entity_key(entity);
        // the plugin's engine: one per thread at a time
        let engine = world.non_send::<ScriptEngine>().0.clone();
        with_world_scope(world, || engine.load("data", DATA_JS)).unwrap();
        let result = with_world_scope(world, || engine.call("data", "run", 1, Some(&e), "{}", "null"));
        assert!(!result.unwrap_or_else(|e| panic!("{e}")));
        app.update();

        let world = app.world();
        let painted = &world.get::<Painted>(entity).unwrap().0;
        let paint = world.resource::<Assets<Paint>>().get(painted).expect("the paint asset was made");
        assert_eq!((paint.shade, paint.name.as_str()), (2.5, "inline"));
        assert_eq!(world.get::<Probe>(entity).unwrap().label, "bundled");
        assert!(world.entity(entity).contains::<Gone>());
    }

    const LIVE_DATA_JS: &str = r#"
exports.make = (e) => insert(e, "Painted", { shade: 1, name: "inline" });
exports.read = function (e) {
  if (resource("Unset") !== null) throw new Error("a missing resource is null");
  const handle = component(e, "Painted")[0];
  if (!handle.startsWith("asset:")) throw new Error(`a path-less handle reads as its id: ${handle}`);
  insert(e, "Painted", get(e, "Painted"));
  const paint = asset(handle, "Paint");
  return paint.shade + paint.name.length;
};
exports.write = function (e) {
  const weather = resource("Weather");
  weather.wind += 1;
  asset(component(e, "Painted")[0], "Paint").shade = weather.wind;
};"#;

    // CLAUDE
    /// resources and assets through live refs. reading an asset doesn't mark it modified
    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn live_resources_and_assets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), plugin))
            .init_asset::<Paint>()
            .register_asset_reflect::<Paint>()
            .register_type::<Painted>()
            .register_type::<Weather>()
            .register_type::<Unset>()
            .insert_resource(Weather { wind: 2.5 });
        values::register_inline::<Paint>(&mut app);
        let entity = app.world_mut().spawn_empty().id();
        let e = entity_key(entity);
        let engine = app.world().non_send::<ScriptEngine>().0.clone();
        let run = |app: &mut App, function: &str| {
            let world = app.world_mut();
            with_world_scope(world, || engine.load("live_data", LIVE_DATA_JS)).unwrap();
            with_world_scope(world, || engine.call("live_data", function, 1, Some(&e), "{}", "null")).unwrap_or_else(|e| panic!("{function}: {e}"));
            app.update();
        };
        let modified = |app: &mut App| {
            let mut events = app.world_mut().resource_mut::<Messages<AssetEvent<Paint>>>();
            let modified = events.drain().filter(|e| matches!(e, AssetEvent::Modified { .. })).count();
            modified
        };

        run(&mut app, "make");
        modified(&mut app);
        run(&mut app, "read");
        assert_eq!(modified(&mut app), 0, "reads leave the asset unmodified");
        run(&mut app, "write");
        assert_eq!(modified(&mut app), 1);

        let world = app.world();
        assert_eq!(world.resource::<Weather>().wind, 3.5);
        let painted = &world.get::<Painted>(entity).unwrap().0;
        assert_eq!(world.resource::<Assets<Paint>>().get(painted).unwrap().shade, 3.5);
    }

    // CLAUDE
    /// math values and live refs outlive the call that made them: generators keep them across yields
    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn values_survive_yields() {
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        world.resource::<AppTypeRegistry>().write().register::<Transform>();
        let entity = world.spawn(Transform::default()).id();
        let e = entity_key(entity);
        let js = js::Js::new().unwrap();
        let source = r#"
exports.run = function* (e) {
  const offset = Vec3.new(1, 2, 3);
  const live = component(e, "Transform");
  yield;
  live.translation = live.translation.add(offset);
};"#;
        with_world_scope(&mut world, || js.load("held", source)).unwrap();
        assert!(with_world_scope(&mut world, || js.call("held", "run", 1, Some(&e), "{}", "null")).unwrap());
        assert_eq!(with_world_scope(&mut world, || js.step(1, 0.0)).unwrap(), Some(0.0));
        assert_eq!(with_world_scope(&mut world, || js.step(1, 0.0)).unwrap(), None);
        assert_eq!(world.get::<Transform>(entity).unwrap().translation, Vec3::new(1.0, 2.0, 3.0));
    }

    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn host_errors_reach_the_script() {
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        let e = entity_key(world.spawn_empty().id());
        let js = js::Js::new().unwrap();
        with_world_scope(&mut world, || js.load("bad", r#"exports.f = (e) => insert(e, "Nope", {});"#)).unwrap();
        let err = with_world_scope(&mut world, || js.call("bad", "f", 1, Some(&e), "{}", "null")).unwrap_err();
        assert!(err.contains("unknown type `Nope`"), "{err}");
    }
}
