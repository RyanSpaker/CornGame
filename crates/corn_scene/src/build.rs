//! CLAUDE (claude-opus-5-5): building a scene file's scene. The loader parses the file and waits for its
//! includes, then hands it to [`serve`], on the main thread (scripts run there), which builds the scene in
//! a world of its own and hands that back to become the file's `WorldAsset`.
//!
//! Building, on the file's root entity ([`SceneFileRoot`]):
//! 1. the file's script module loads: its top-level code runs
//! 2. `asset` items are made, in order
//! 3. in source order: patches go on the entity, `class:` adds names, `spawn` makes children (applying
//!    their bodies), `include` brings its content onto the entity then applies its body
//! 4. rules give out their classes, so `#a { class: x; }` and `.x { }` work in either order
//! 5. the rules run in source order, nested rules right after their parent. each rule first collects its
//!    matches among the scope and its descendants, then applies its body to each, so what a rule spawns
//!    is not matched by that same rule
//! 6. every ancestor of an entity with a `Transform` (or `Visibility`, often required by `Mesh3d`) gets a
//!    default one, if it has none
//!
//! Including, with `as child` keeping what's included a child of the entity instead:
//! - a `.corn` file pastes its items onto the entity: they apply as step 3 does, its rules join the queue
//!   with the entity as their scope, so they see everything under it, whatever made it. `file.corn#name`
//!   pastes the body of `spawn name` and the file's top-level rules. The file also loaded on its own,
//!   once: its script module and assets are that load's, shared by everything including it, and `root` in
//!   its `${ .. }` is the entity it's pasted on
//! - anything else (gltf) copies that scene in, before the body's other items: its root becomes the entity
//!   (all but `Name` and the hierarchy), so it's what patches apply over. several roots need `as child`
//!
//! Selecting:
//! - `#name` names only what the selector's own file spawned ([`Spawned`]), and in an include's body also
//!   what that include brought in: `include "x.gltf" { #lamp { .. } }`. Anywhere else, `[Name=lamp]`
//!   selects by name whatever made it. What a file's `script { .. }` or `${ .. }` spawn while it builds is
//!   the file's too; what scripts spawn later has no file, so only `[Name=..]` selects it. Scripts'
//!   `select(e, ..)` names as the calling script's file does
//! - everything else (components, classes, attributes, relations) selects across includes
//! - `:root` starts at the root of the scene being built: `select(:root [Name=level])`
//! - `A /Comp/ B`: B is an entity A's `Comp` holds (`#ring /Listeners/ *`, `#switch /Target/ *`); for a
//!   relationship component, B is one whose `Comp` points at A (`#ring /ListenerOf/ *`). Both are in the
//!   rule's scope
//!
//! Which write wins, per field of a component on an entity ([`Rank`]). Like css layers, each pasted file
//! is a layer:
//! - a file beats what it includes. two files included side by side: the later include wins
//! - in one file, a direct patch beats a rule (`spawn x { class: a; Kind: Box; }` keeps `Box`), and between
//!   rules the later one wins. there's no selector specificity yet
//! - `default` only fills what nothing wrote: `Health: default` keeps `Health.max: 1` and an existing
//!   `Health`. scene data (a gltf's) ranks below any write: a scene that comes after `Health.max: 1`
//!   (in whatever file) keeps that field and fills the others
//!
//! `${ .. }` values run as they apply, once per entity, and see the scene as patched so far. One that lost
//! to a higher rank doesn't run. A patch that fails fails the load.
//!
//! Entity references (`Target: #door;`, `Lamps: select(.lamp);`) are selectors over the file's scope: the
//! entity the file applies to (its root, or the entity it's pasted on) and its descendants, like the
//! file's top-level rules. A single `Entity` must match exactly one entity. The fields holding them are
//! written after the rules, so a reference may name what a later item or a rule spawns, and `${ .. }` and
//! selectors don't see them while building. They still rank as where they're written. `[Comp.field=#a]`
//! selects entities whose field is one of what `#a` matches, so `[ChildOf=#ui]` is `#ui > *`.
//!
//! Double spawning: everything `spawn` makes carries [`Spawned`] (file and item). Before spawning, the
//! parent's children are checked for it, so applying the same item to the same entity again (a rule
//! matching again, a file included twice) spawns nothing new.
//!
//! A scene that spawns itself through `WorldAssetRoot` fails to load, see [`ChildScenes`].

use std::any::TypeId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use bevy::ecs::entity::{EntityHashMap, EntityMapper, SceneEntityMapper};
use bevy::ecs::relationship::{RelationshipAccessor, RelationshipHookMode};
use bevy::ecs::resource::IS_RESOURCE;
use bevy::ecs::world::EntityRef;
use bevy::prelude::*;
use bevy::reflect::{ReflectFromReflect, ReflectMut, ReflectRef, TypeRegistration, TypeRegistry};
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use corn_assets::ReflectInlineAsset;

use crate::doc::{split_include, AttrOp, Body, Combinator, Compound, Doc, Handler, Item, Selector};
use crate::value::{default_of, holds_entity, patch, patch_path, type_at, EntityRefs, ParseCx, Value};
use crate::CornFile;

/// the entity a scene file's top-level items apply to
#[derive(Component, Reflect, Default, Debug, Clone, Copy)]
#[reflect(Component, Default)]
pub struct SceneFileRoot;

/// classes applied to this entity, for `.class` selectors
#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component, Default)]
pub struct Classes(pub Vec<String>);

/// made by a `spawn` item: which file (a hash of its path) and which item. Or by the file's scripts while
/// it built: key [`Spawned::SCRIPTED`]
#[derive(Component, Reflect, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct Spawned {
    pub file: u64,
    pub key: u32,
}

impl Spawned {
    /// the key of what a file's `script { .. }` or `${ .. }` spawned while it built: no item's key
    pub const SCRIPTED: u32 = u32::MAX;
}

// CLAUDE
/// what `include` brought onto this entity, by the paths as written. `[Included~="level.gltf#Scene0"]`
/// selects by it
#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component, Default)]
pub struct Included(pub Vec<String>);

/// `on` items on this entity, which become script listeners once it is spawned
#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component, Default)]
pub struct SceneListeners(pub Vec<SceneListener>);

#[derive(Reflect, Default, Debug, Clone, PartialEq)]
pub struct SceneListener {
    /// the event's type path
    pub event: String,
    pub module: String,
    pub function: String,
    /// lifecycle events: the component's type path. empty otherwise
    pub component: String,
}

// CLAUDE
/// what an `include` path loaded
pub(crate) enum Include {
    Scene(Handle<WorldAsset>),
    File(Handle<CornFile>),
}

// CLAUDE
/// a parsed file and what it refers to: enough to build it, or paste it into another file
pub(crate) struct Source {
    pub doc: Arc<Doc>,
    /// the file's script module, `file#$script`
    pub module: String,
    pub includes: HashMap<String, Include>,
    /// handles for the `asset` items, filled by the loader once they're made
    pub named: HashMap<String, UntypedHandle>,
}

/// a file to build
pub(crate) struct Request {
    pub source: Arc<Source>,
    pub reply: async_channel::Sender<Result<Built, String>>,
}

pub(crate) struct Built {
    pub world: World,
    /// the `asset` items' data: name, asset type, value of its data type
    pub assets: Vec<(String, TypeId, Box<dyn PartialReflect>)>,
    /// the top-level `spawn` items, each a scene of its own
    pub fragments: Vec<(String, World)>,
}

// CLAUDE
/// which scenes each built scene spawns through `WorldAssetRoot`, by path, to catch one spawning itself.
/// scripts inserting `WorldAssetRoot` at runtime aren't seen
#[derive(Resource, Default)]
pub(crate) struct ChildScenes(pub HashMap<String, Vec<String>>);

impl ChildScenes {
    /// records what scenes spawn, an error if that closes a loop
    fn record(&mut self, scenes: Vec<(String, Vec<String>)>) -> Result<(), String> {
        let names: Vec<String> = scenes.iter().map(|(name, _)| name.clone()).collect();
        self.0.extend(scenes);
        for name in &names {
            let mut stack = vec![vec![name.clone()]];
            let mut seen = HashSet::new();
            while let Some(chain) = stack.pop() {
                for next in self.0.get(chain.last().unwrap()).into_iter().flatten() {
                    let mut chain = chain.clone();
                    chain.push(next.clone());
                    if next == name {
                        names.iter().for_each(|n| drop(self.0.remove(n)));
                        return Err(format!("a scene spawns itself: {}", chain.join(" -> ")));
                    }
                    if seen.insert(next.clone()) {
                        stack.push(chain);
                    }
                }
            }
        }
        Ok(())
    }
}

fn child_scenes(world: &mut World) -> Vec<String> {
    let mut roots = world.query::<&WorldAssetRoot>();
    roots.iter(world).filter_map(|root| root.0.path().map(|p| p.to_string())).collect()
}

#[derive(Resource)]
pub(crate) struct Requests(pub async_channel::Receiver<Request>);

/// builds the files the loader is waiting for
pub(crate) fn serve(world: &mut World) {
    let requests = world.resource::<Requests>().0.clone();
    while let Ok(request) = requests.try_recv() {
        let reply = request.reply.clone();
        let _ = reply.try_send(build(world, request));
    }
}

pub(crate) fn file_hash(path: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

fn build(app: &mut World, request: Request) -> Result<Built, String> {
    let registry = app.resource::<AppTypeRegistry>().clone();
    let assets = app.resource::<AssetServer>().clone();
    let mut scene = World::new();
    let root = scene.spawn(SceneFileRoot).id();
    let doc = request.source.doc.clone();
    let at = Applying { source: request.source, root, layer: Arc::default(), rule: false, included: None };

    let mut builder = Builder {
        app,
        scene: &mut scene,
        registry: registry.clone(),
        assets,
        rules: VecDeque::new(),
        new_rules: false,
        rule_classes: HashSet::new(),
        written: HashMap::new(),
        deferred: vec![],
        next_layer: 0,
        copied: HashMap::new(),
        build_root: root,
        errors: vec![],
    };
    // CLAUDE: stage times, for `debug!` at the end (scripts' host calls too, with corn_scripting's `profile`)
    #[cfg(feature = "scripting")]
    let _ = corn_scripting::profile::take();
    let mut stages = vec![];
    // CODEX (GPT-6): Bevy's clock uses performance.now in browsers and std time on native.
    let mut clock = bevy::platform::time::Instant::now();
    let mut lap = |stage: &'static str| {
        stages.push((stage, clock.elapsed()));
        clock = bevy::platform::time::Instant::now();
    };
    builder.load_module(&at)?;
    lap("module");
    let made = builder.make_assets(&at)?;
    lap("assets");
    builder.build(&at, root, &doc.body);
    lap("items");
    builder.run_rules();
    builder.apply_deferred();
    lap("rules");
    let (errors, rule_classes) = (std::mem::take(&mut builder.errors), std::mem::take(&mut builder.rule_classes));
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    for path in ["bevy_transform::components::transform::Transform", "bevy_camera::visibility::Visibility"] {
        fill_parents(app, &mut scene, &registry.read(), path);
    }
    lap("parents");

    for warning in near_miss_classes(&mut scene, &rule_classes) {
        warn!("{}: {warning}", doc.path);
    }
    let registry = registry.read();
    let unregistered = unregistered_components(&scene, &registry);
    if !unregistered.is_empty() {
        return Err(format!(
            "{}: the scene has components that aren't reflected (#[reflect(Component)], registered), so it can't spawn: {}",
            doc.path,
            unregistered.join(", ")
        ));
    }
    for warning in check_entity_mapping(&scene, &registry) {
        warn!("{}: {warning}", doc.path);
    }
    let file = file_hash(&doc.path);
    let mut fragments: Vec<(String, World)> = doc
        .fragments()
        .filter_map(|(name, key)| {
            let spawned = Spawned { file, key };
            let children = scene.get::<Children>(root)?;
            let entity = children.iter().find(|&c| scene.get::<Spawned>(c) == Some(&spawned))?;
            Some((name.to_string(), copy_subtree(&scene, entity, &registry)))
        })
        .collect();
    let mut scenes = vec![(doc.path.clone(), child_scenes(&mut scene))];
    for (name, world) in &mut fragments {
        scenes.push((format!("{}#{name}", doc.path), child_scenes(world)));
    }
    app.resource_mut::<ChildScenes>().record(scenes).map_err(|e| format!("{}: {e}", doc.path))?;
    lap("finish");
    let total: std::time::Duration = stages.iter().map(|(_, t)| *t).sum();
    let stages: Vec<String> = stages.iter().map(|(stage, t)| format!("{stage} {:.2}ms", t.as_secs_f64() * 1e3)).collect();
    #[cfg(feature = "scripting")]
    let scripts = corn_scripting::profile::describe(&corn_scripting::profile::take());
    #[cfg(not(feature = "scripting"))]
    let scripts = String::new();
    debug!("built {} in {:.2}ms: {}; scripts: {scripts}", doc.path, total.as_secs_f64() * 1e3, stages.join(", "));
    Ok(Built { world: scene, assets: made, fragments })
}

// CLAUDE
/// gives the component `type_path` names, default, to every ancestor of an entity that has it or will (one of
/// its components requires it in the app: `Mesh3d` requires Transform, and Visibility through a plugin, which
/// the scene's own world doesn't know), so a mesh under a bare `spawn` moves and hides with it. bevy only
/// warns (B0004) and BSN leaves it to the author. The file's root gets it too
fn fill_parents(app: &World, scene: &mut World, registry: &TypeRegistry, type_path: &str) {
    // Visibility only if the app has it (corn_scene doesn't depend on bevy_camera)
    let Some(registration) = registry.get_with_type_path(type_path) else { return };
    let (Some(reflect), Some(default)) = (registration.data::<ReflectComponent>(), registration.data::<ReflectDefault>()) else { return };
    let wanted = registration.type_id();
    // the scene's component types that require it in the app
    let requires = |type_id: TypeId| {
        let Some(info) = app.components().get_id(type_id).and_then(|id| app.components().get_info(id)) else { return false };
        info.required_components().iter_ids().any(|r| app.components().get_info(r).and_then(|i| i.type_id()) == Some(wanted))
    };
    let has: HashSet<_> = scene
        .components()
        .iter_registered()
        .filter(|(_, info)| info.type_id().is_some_and(|t| t == wanted || requires(t)))
        .map(|(id, _)| id)
        .collect();
    if has.is_empty() {
        return;
    }
    let holds = |e: EntityRef| e.archetype().components().iter().any(|c| has.contains(c));
    let mut missing = HashSet::new();
    let with: Vec<Entity> = scene.iter_entities().filter(|&e| holds(e)).map(|e| e.id()).collect();
    for e in with {
        let mut up = parent(scene, e);
        while let Some(p) = up {
            if holds(scene.entity(p)) || !missing.insert(p) {
                break;
            }
            up = parent(scene, p);
        }
    }
    for e in missing {
        reflect.insert(&mut scene.entity_mut(e), default.default().as_partial_reflect(), registry);
    }
}

// CLAUDE
/// the file items being applied came from, and the entity it's applied to
#[derive(Clone)]
struct Applying {
    source: Arc<Source>,
    root: Entity,
    /// the includes that pasted the file, outermost first, numbered in the order they were pasted
    layer: Arc<Vec<u32>>,
    /// whether the items are a rule's body, rather than direct patches
    rule: bool,
    /// in an include's body: what it included (a hash of the file, see [`Matcher::names`]), whose `#name`s
    /// the body names too
    included: Option<u64>,
}

impl Applying {
    fn direct(&self) -> Applying {
        Applying { rule: false, ..self.clone() }
    }

    fn rank(&self) -> Rank {
        Rank { layer: self.layer.clone(), direct: !self.rule }
    }
}

// CLAUDE
/// who wrote a field, see the module docs
#[derive(Clone, Debug)]
struct Rank {
    layer: Arc<Vec<u32>>,
    direct: bool,
}

impl Rank {
    /// whether a write of this rank keeps out one of `other`. on a tie the later write wins, so no
    fn beats(&self, other: &Rank) -> bool {
        for (a, b) in self.layer.iter().zip(other.layer.iter()) {
            if a != b {
                return a > b;
            }
        }
        if self.layer.len() != other.layer.len() {
            return self.layer.len() < other.layer.len();
        }
        self.direct && !other.direct
    }
}


fn cx<'c>(
    named: &'c HashMap<String, UntypedHandle>,
    registry: &'c TypeRegistry,
    assets: &'c AssetServer,
    entities: Option<&'c EntityRefs>,
) -> ParseCx<'c> {
    ParseCx {
        registry,
        assets: Some(assets),
        named: Some(named),
        #[cfg(feature = "scripting")]
        json: Some(corn_scripting::apply_json),
        #[cfg(not(feature = "scripting"))]
        json: None,
        entities,
    }
}

// CLAUDE
/// a patch holding entity references, applied after the rules
struct Deferred {
    at: Applying,
    entity: Entity,
    component: TypeId,
    field: Vec<String>,
    value: Value,
    /// the length of `written` for the component once this patch was recorded: an overlapping write after
    /// that won
    after: usize,
    /// `file:line:col: Comp.field`, for errors
    what: String,
}

struct Builder<'a> {
    app: &'a mut World,
    scene: &'a mut World,
    registry: AppTypeRegistry,
    assets: AssetServer,
    /// rules to run: where they came from, scope, the body holding the rule, its index
    rules: VecDeque<(Applying, Entity, Body, usize)>,
    /// rules were queued since rules last gave out their classes
    new_rules: bool,
    /// the classes rules select by
    rule_classes: HashSet<String>,
    /// which fields have been written, and by whom
    written: HashMap<(Entity, TypeId), Vec<(Vec<String>, Rank)>>,
    deferred: Vec<Deferred>,
    next_layer: u32,
    /// the entities included scenes (gltf) copied in, and which (a hash of the include's path), so their
    /// include's body can name them
    copied: HashMap<Entity, u64>,
    /// the root of the scene being built, for `:root`
    build_root: Entity,
    /// what fails the load
    errors: Vec<String>,
}


impl Builder<'_> {
    fn load_module(&mut self, at: &Applying) -> Result<(), String> {
        let doc = &at.source.doc;
        let Some(source) = doc.module_source() else { return Ok(()) };
        #[cfg(feature = "scripting")]
        {
            let before = self.entity_set();
            let loaded = corn_scripting::load_scene_module(self.app, self.scene, &at.source.module, &source)
                .map_err(|e| format!("{}: script: {e}", doc.path));
            self.claim(at, &before);
            loaded
        }
        #[cfg(not(feature = "scripting"))]
        {
            let _ = source;
            Err(format!("{}: scripts need corn_scene's `scripting` feature", doc.path))
        }
    }

    /// runs the `${ .. }` in `value` for `entity`
    fn resolve(&mut self, at: &Applying, value: &Value, entity: Entity) -> Result<Value, String> {
        if !value.has_script() {
            return Ok(value.clone());
        }
        #[cfg(feature = "scripting")]
        {
            let before = self.entity_set();
            let (app, scene) = (&mut *self.app, &mut *self.scene);
            let source = &at.source;
            let resolved = value.resolve_scripts(&mut |index| {
                let function = source.doc.inline_function(index);
                corn_scripting::eval_in_scene(app, scene, &source.module, &function, entity, at.root)
                    .map_err(|e| format!("${{..}} at {}: {e}", source.doc.inline[index].loc))
            });
            self.claim(at, &before);
            resolved
        }
        #[cfg(not(feature = "scripting"))]
        {
            let _ = (at, entity);
            Err("scripts need corn_scene's `scripting` feature".into())
        }
    }

    // CLAUDE
    /// the `asset` items' data, in order
    fn make_assets(&mut self, at: &Applying) -> Result<Vec<(String, TypeId, Box<dyn PartialReflect>)>, String> {
        let mut made = vec![];
        for item in at.source.doc.body.iter() {
            let Item::Asset { name, asset, type_name, value, loc } = item else { continue };
            let fail = |e: String| format!("{}:{loc}: asset {name}: {e}", at.source.doc.path);
            let value = self.resolve(at, value, at.root).map_err(fail)?;
            let registry = self.registry.clone();
            let registry = registry.read();
            let inline = registry.get_type_data::<ReflectInlineAsset>(*asset).ok_or_else(|| {
                fail(format!("`{type_name}` can't be made from data (register it with corn_assets::register_inline or register_recipe)"))
            })?;
            let info = registry.get_type_info(inline.data).ok_or_else(|| fail("its data type is not registered".into()))?;
            let mut data = default_of(info, &registry).map_err(fail)?;
            let assets = self.assets.clone();
            patch(data.as_mut(), &value, &cx(&at.source.named, &registry, &assets, None)).map_err(fail)?;
            made.push((name.clone(), *asset, data));
        }
        Ok(made)
    }

    fn build(&mut self, at: &Applying, entity: Entity, body: &Body) {
        let doc = at.source.doc.clone();
        let path = &doc.path;
        // scenes first: they're what patches apply over
        for item in body.iter() {
            let Item::Include { path: included, child: false, loc, .. } = item else { continue };
            if let Some(Include::Scene(handle)) = at.source.includes.get(included) {
                if let Err(e) = self.copy_scene(handle, file_hash(included)).and_then(|roots| self.become_root(entity, roots, included)) {
                    self.errors.push(format!("{path}:{loc}: include \"{included}\": {e}"));
                }
            }
        }
        for (index, item) in body.iter().enumerate() {
            match item {
                Item::Patch { component, name, field, value, loc } => {
                    let key = (entity, *component);
                    let Some(value) = self.winning(key, field, value, &at.rank()) else { continue };
                    // `Comp: default` attaches it, it doesn't reset it
                    if field.is_empty() && matches!(&value, Value::Path(p) if p == "default") && self.scene.entity(entity).contains_type_id(*component) {
                        continue;
                    }
                    let what = format!("{path}:{loc}: {name}{}", field.iter().map(|f| format!(".{f}")).collect::<String>());
                    let (now, later) = self.split_refs(*component, field, value);
                    let after = self.written.get(&key).map_or(0, Vec::len);
                    for (field, value) in later {
                        self.deferred.push(Deferred { at: at.clone(), entity, component: *component, field, value, after, what: what.clone() });
                    }
                    let Some(value) = now else { continue };
                    let result = self.resolve(at, &value, entity).and_then(|value| self.apply_patch(at, entity, *component, field, &value, None));
                    if let Err(e) = result {
                        self.errors.push(format!("{what}: {e}"));
                    }
                }
                Item::Classes { names, .. } => {
                    self.add_classes(entity, names);
                }
                Item::Spawn { name, body, key, .. } => {
                    let spawned = Spawned { file: file_hash(path), key: *key };
                    if already_spawned(self.scene, entity, spawned) {
                        continue;
                    }
                    let child = self.scene.spawn((Name::new(name.clone()), spawned, ChildOf(entity))).id();
                    self.build(&at.direct(), child, body);
                }
                Item::Include { path: included, child, body, loc } => match self.include(at, entity, included, *child) {
                    // the body names what's included
                    Ok((targets, file)) => {
                        let inside = Applying { included: Some(file), ..at.clone() };
                        for target in targets {
                            if let Some(body) = body {
                                self.build(&inside, target, body);
                            }
                        }
                    }
                    Err(e) => self.errors.push(format!("{path}:{loc}: include \"{included}\": {e}")),
                },
                Item::Rule { .. } => self.queue_rule(at, entity, body, index),
                Item::On { event, component, handler, loc, .. } => {
                    let registry = self.registry.read();
                    let type_path = |id: TypeId| registry.get(id).map(|r| r.type_info().type_path().to_string());
                    let Some(event) = type_path(*event) else { continue };
                    let component = component.and_then(type_path).unwrap_or_default();
                    if cfg!(not(feature = "scripting")) {
                        warn!("{path}:{loc}: scripts need corn_scene's `scripting` feature");
                        continue;
                    }
                    let function = match handler {
                        Handler::Export(name) => name.clone(),
                        Handler::Inline(index) => doc.inline_function(*index),
                    };
                    let listener = SceneListener { event, module: at.source.module.clone(), function, component };
                    let mut e = self.scene.entity_mut(entity);
                    match e.get_mut::<SceneListeners>() {
                        Some(mut listeners) => listeners.0.push(listener),
                        None => {
                            e.insert(SceneListeners(vec![listener]));
                        }
                    }
                }
                Item::Script { index, loc } => {
                    if let Err(e) = self.run(at, *index, entity) {
                        self.errors.push(format!("{path}:{loc}: script: {e}"));
                    }
                }
                // made before building
                Item::Asset { .. } => {}
            }
        }
    }

    // CLAUDE
    /// runs `script { .. }` number `index` for `entity`
    fn run(&mut self, at: &Applying, index: usize, entity: Entity) -> Result<(), String> {
        #[cfg(feature = "scripting")]
        {
            let before = self.entity_set();
            let function = at.source.doc.inline_function(index);
            let ran = corn_scripting::eval_in_scene(self.app, self.scene, &at.source.module, &function, entity, at.root).map(|_| ());
            self.claim(at, &before);
            ran
        }
        #[cfg(not(feature = "scripting"))]
        {
            let _ = (at, index, entity);
            Err("scripts need corn_scene's `scripting` feature".into())
        }
    }

    // CLAUDE
    #[cfg(feature = "scripting")]
    fn entity_set(&self) -> HashSet<Entity> {
        self.scene.iter_entities().map(|e| e.id()).collect()
    }

    // CLAUDE
    /// makes what a script of `at`'s file spawned (what isn't in `before`) the file's, so its `#name`s name it
    #[cfg(feature = "scripting")]
    fn claim(&mut self, at: &Applying, before: &HashSet<Entity>) {
        let spawned = Spawned { file: file_hash(&at.source.doc.path), key: Spawned::SCRIPTED };
        let new: Vec<Entity> = self.scene.iter_entities().filter(|e| !before.contains(&e.id()) && !e.contains::<Spawned>()).map(|e| e.id()).collect();
        for entity in new {
            self.scene.entity_mut(entity).insert(spawned);
        }
    }

    // CLAUDE
    /// brings an included scene or file onto `entity`, see the module docs. gives the entities the
    /// include's body applies to, and what it names (see [`Applying::included`])
    fn include(&mut self, at: &Applying, entity: Entity, path: &str, child: bool) -> Result<(Vec<Entity>, u64), String> {
        let (targets, file) = match at.source.includes.get(path).ok_or("not loaded")? {
            // copied before the body's items
            Include::Scene(_) if !child => (vec![entity], file_hash(path)),
            Include::Scene(handle) => {
                let roots = self.copy_scene(handle, file_hash(path))?;
                for &root in &roots {
                    self.scene.entity_mut(root).insert(ChildOf(entity));
                }
                (roots, file_hash(path))
            }
            Include::File(handle) => {
                let target = if child { self.scene.spawn(ChildOf(entity)).id() } else { entity };
                let file = self.paste(at, handle, target, path)?;
                (vec![target], file)
            }
        };
        let mut e = self.scene.entity_mut(entity);
        match e.get_mut::<Included>() {
            Some(mut included) if !included.0.iter().any(|p| p == path) => included.0.push(path.to_string()),
            Some(_) => {}
            None => {
                e.insert(Included(vec![path.to_string()]));
            }
        }
        Ok((targets, file))
    }

    /// copies a scene into the scene being built, giving its roots. `file`: which, see [`Builder::copied`]
    fn copy_scene(&mut self, handle: &Handle<WorldAsset>, file: u64) -> Result<Vec<Entity>, String> {
        let source = self.app.resource::<Assets<WorldAsset>>().get(handle).ok_or("not loaded")?;
        let mut map = EntityHashMap::default();
        source.write_to_world_with(self.scene, &mut map, &self.registry).map_err(|e| e.to_string())?;
        self.copied.extend(map.values().map(|&e| (e, file)));
        Ok(map.values().copied().filter(|&e| self.scene.get::<ChildOf>(e).is_none()).collect())
    }

    fn become_root(&mut self, entity: Entity, roots: Vec<Entity>, path: &str) -> Result<(), String> {
        match roots[..] {
            [] => Ok(()),
            [root] => {
                self.merge(root, entity);
                Ok(())
            }
            _ => Err(format!("it has {} roots, so it can't become this entity: `include \"{path}\" as child`", roots.len())),
        }
    }

    /// applies a file's items to `target`, as a layer inside the current one. gives the file's hash
    fn paste(&mut self, at: &Applying, handle: &Handle<CornFile>, target: Entity, path: &str) -> Result<u64, String> {
        let source = self.app.resource::<Assets<CornFile>>().get(handle).ok_or("not loaded")?.source.clone();
        self.next_layer += 1;
        let mut layer = at.layer.to_vec();
        layer.push(self.next_layer);
        let pasted = Applying { source: source.clone(), root: target, layer: Arc::new(layer), rule: false, included: None };
        let doc = &source.doc;
        let file = file_hash(&doc.path);
        let Some(label) = split_include(path).1 else {
            self.build(&pasted, target, &doc.body);
            return Ok(file);
        };
        let body = doc.body.iter().find_map(|item| match item {
            Item::Spawn { name, body, .. } if name == label => Some(body.clone()),
            _ => None,
        });
        self.build(&pasted, target, &body.ok_or_else(|| format!("no top-level `spawn {label}` there"))?);
        for (index, item) in doc.body.iter().enumerate() {
            if matches!(item, Item::Rule { .. }) {
                self.queue_rule(&pasted, target, &doc.body, index);
            }
        }
        Ok(file)
    }

    fn add_classes(&mut self, entity: Entity, names: &[String]) -> bool {
        let mut e = self.scene.entity_mut(entity);
        if e.get::<Classes>().is_none() {
            e.insert(Classes::default());
        }
        let mut classes = e.get_mut::<Classes>().unwrap();
        let mut added = false;
        for name in names {
            if !classes.0.contains(name) {
                classes.0.push(name.clone());
                added = true;
            }
        }
        added
    }

    fn queue_rule(&mut self, at: &Applying, scope: Entity, body: &Body, index: usize) {
        let Item::Rule { selectors, .. } = &body[index] else { unreachable!() };
        for part in selectors.iter().flat_map(|s| &s.parts) {
            self.rule_classes.extend(part.1.classes.iter().cloned());
        }
        self.rules.push_back((at.clone(), scope, body.clone(), index));
        self.new_rules = true;
    }

    // CLAUDE
    /// `value` without what higher-ranked writes hold, recording the rest as written by `rank`. None if
    /// nothing's left
    fn winning(&mut self, key: (Entity, TypeId), path: &[String], value: &Value, rank: &Rank) -> Option<Value> {
        if let Value::Struct(name, entries) = value {
            if name.is_empty() && !entries.is_empty() {
                let kept: Vec<_> = entries
                    .iter()
                    .filter_map(|(field, v)| Some((field.clone(), self.winning(key, &[path, field].concat(), v, rank)?)))
                    .collect();
                return (!kept.is_empty()).then(|| Value::Struct(name.clone(), kept));
            }
        }
        let written = self.written.entry(key).or_default();
        let mut overlapping = written.iter().filter(|(w, _)| w.starts_with(path) || path.starts_with(w)).peekable();
        if matches!(value, Value::Path(p) if p == "default") {
            return overlapping.peek().is_none().then(|| value.clone());
        }
        if overlapping.any(|(_, r)| r.beats(rank)) {
            return None;
        }
        written.push((path.to_vec(), rank.clone()));
        Some(value.clone())
    }

    /// moves `from`'s components (but its name and its hierarchy) and children onto `to`, despawning `from`.
    /// fields written on `to` stay, the scene's value fills the rest. children `to` already has a copy of (by
    /// [`Spawned`]) are dropped
    fn merge(&mut self, from: Entity, to: Entity) {
        let registry = self.registry.read();
        let skip = [TypeId::of::<ChildOf>(), TypeId::of::<Children>(), TypeId::of::<Name>()];
        let types: Vec<TypeId> = self.scene.inspect_entity(from).unwrap().filter_map(|(_, info)| info.type_id()).collect();
        for type_id in types {
            if skip.contains(&type_id) {
                continue;
            }
            let Some(registration) = registry.get(type_id) else { continue };
            let Some(component) = registration.data::<ReflectComponent>() else { continue };
            let Some(mut value) = component.reflect(self.scene.entity(from)).map(|v| clone_value(v.as_partial_reflect(), registration)) else {
                continue;
            };
            let written: Vec<&Vec<String>> = self.written.get(&(to, type_id)).into_iter().flatten().map(|(path, _)| path).collect();
            // (written but not there: an entity reference, written later)
            if let Some(current) = component.reflect(self.scene.entity(to)).filter(|_| !written.is_empty()) {
                let current = clone_value(current.as_partial_reflect(), registration);
                // the whole component written, or fields of another enum variant: `to`'s stays
                let kept = written.iter().all(|path| copy_field(current.as_ref(), value.as_mut(), path));
                if !kept {
                    continue;
                }
            }
            if type_id == TypeId::of::<Classes>() {
                let added = Classes::from_reflect(value.as_ref()).unwrap_or_default();
                if let Some(mut classes) = self.scene.get_mut::<Classes>(to) {
                    for class in added.0 {
                        if !classes.0.contains(&class) {
                            classes.0.push(class);
                        }
                    }
                    continue;
                }
            }
            component.insert(&mut self.scene.entity_mut(to), value.as_ref(), &registry);
        }
        let children: Vec<Entity> = self.scene.get::<Children>(from).map(|c| c.to_vec()).unwrap_or_default();
        for child in children {
            match self.scene.get::<Spawned>(child).copied() {
                Some(spawned) if spawned.key != Spawned::SCRIPTED && already_spawned(self.scene, to, spawned) => self.scene.entity_mut(child).despawn(),
                _ => {
                    self.scene.entity_mut(child).insert(ChildOf(to));
                }
            }
        }
        self.scene.entity_mut(from).despawn();
    }

    // CLAUDE
    /// `value` for a patch at `path`: what applies now, and the parts holding entity references (field path,
    /// value) to apply after the rules. a `#name` in a field that can't hold an entity is a color
    fn split_refs(&self, component: TypeId, path: &[String], value: Value) -> (Option<Value>, Vec<(Vec<String>, Value)>) {
        if !value.has_ref() {
            return (Some(value), vec![]);
        }
        if let Value::Struct(name, entries) = &value {
            if name.is_empty() {
                let (mut now, mut later) = (vec![], vec![]);
                for (field, v) in entries {
                    let (n, l) = self.split_refs(component, &[path, field].concat(), v.clone());
                    now.extend(n.map(|n| (field.clone(), n)));
                    later.extend(l);
                }
                return ((!now.is_empty()).then(|| Value::Struct(name.clone(), now)), later);
            }
        }
        let registry = self.registry.read();
        // a path that isn't there fails when it applies
        let entities = registry.get_type_info(component).and_then(|info| type_at(&registry, info, path)).is_none_or(|info| holds_entity(&registry, info));
        if entities {
            (None, vec![(path.to_vec(), value)])
        } else {
            (Some(value), vec![])
        }
    }

    // CLAUDE
    /// the patches holding entity references, in order. one a later write overlaps lost to it
    fn apply_deferred(&mut self) {
        for d in std::mem::take(&mut self.deferred) {
            let written = self.written.get(&(d.entity, d.component)).map_or(&[][..], |w| &w[d.after.min(w.len())..]);
            if written.iter().any(|(w, _)| w.starts_with(&d.field) || d.field.starts_with(w)) || self.scene.get_entity(d.entity).is_err() {
                continue;
            }
            let result = self.resolve(&d.at, &d.value, d.entity).and_then(|value| {
                let refs = self.entity_refs(&d.at, &value);
                self.apply_patch(&d.at, d.entity, d.component, &d.field, &value, Some(&refs))
            });
            if let Err(e) = result {
                self.errors.push(format!("{}: {e}", d.what));
            }
        }
    }

    // CLAUDE
    /// what each entity reference in `value` matches, in the scope of the file items are `at`
    fn entity_refs(&self, at: &Applying, value: &Value) -> EntityRefs {
        let registry = self.registry.read();
        self.matcher(at, &registry).refs(value)
    }

    fn matcher<'m>(&'m self, at: &Applying, registry: &'m TypeRegistry) -> Matcher<'m> {
        Matcher {
            world: self.scene,
            registry,
            root: at.root,
            build_root: self.build_root,
            file: Some(file_hash(&at.source.doc.path)),
            included: at.included,
            copied: &self.copied,
        }
    }

    fn apply_patch(
        &mut self,
        at: &Applying,
        entity: Entity,
        component: TypeId,
        field: &[String],
        value: &Value,
        refs: Option<&EntityRefs>,
    ) -> Result<(), String> {
        let registry = self.registry.clone();
        let registry = registry.read();
        let assets = self.assets.clone();
        let cx = cx(&at.source.named, &registry, &assets, refs);
        let registration = registry.get(component).ok_or("not registered")?;
        let reflect = registration.data::<ReflectComponent>().ok_or("not a component (missing #[reflect(Component)])")?;
        let world = &mut *self.scene;
        let mutable = world
            .components()
            .get_id(component)
            .and_then(|id| world.components().get_info(id))
            .is_none_or(|info| info.mutable());

        let mut e = world.entity_mut(entity);
        if mutable {
            if let Some(mut existing) = reflect.reflect_mut(&mut e) {
                return patch_path(existing.as_partial_reflect_mut(), field, value, &cx);
            }
        }
        // absent, or immutable: patch a copy and insert it
        let mut new = match reflect.reflect(&e) {
            Some(existing) => existing.reflect_clone().map_err(|err| err.to_string())?.into_partial_reflect(),
            None => default_of(registration.type_info(), &registry)?,
        };
        patch_path(new.as_mut(), field, value, &cx)?;
        reflect.insert(&mut e, new.as_ref(), &registry);
        Ok(())
    }

    fn run_rules(&mut self) {
        while !self.rules.is_empty() {
            if std::mem::take(&mut self.new_rules) {
                self.give_out_classes();
            }
            let rule = self.rules.pop_front().unwrap();
            // rules the body adds (nested ones) run before the remaining ones
            let rest = std::mem::take(&mut self.rules);
            self.apply_rule(rule);
            self.rules.extend(rest);
        }
    }

    // CLAUDE
    /// the `class:` items of the queued rules, until there are no new ones (a rule may select by a class
    /// another one gives)
    fn give_out_classes(&mut self) {
        let rules: Vec<_> = self.rules.iter().map(|(at, scope, body, index)| (at.clone(), *scope, body.clone(), *index)).collect();
        loop {
            let mut added = false;
            for (at, scope, body, index) in &rules {
                let Item::Rule { selectors, body: rule_body, .. } = &body[*index] else { unreachable!() };
                let names: Vec<&Vec<String>> = rule_body.iter().filter_map(|i| if let Item::Classes { names, .. } = i { Some(names) } else { None }).collect();
                if names.is_empty() {
                    continue;
                }
                for entity in self.matches(at, *scope, selectors) {
                    for names in &names {
                        added |= self.add_classes(entity, names);
                    }
                }
            }
            if !added {
                return;
            }
        }
    }

    /// what a rule's `selectors` match in `scope`, for items `at`
    fn matches(&self, at: &Applying, scope: Entity, selectors: &[Selector]) -> Vec<Entity> {
        if unsupported_pseudo(selectors) {
            return vec![];
        }
        let registry = self.registry.read();
        self.matcher(at, &registry).select(scope, selectors)
    }

    fn apply_rule(&mut self, (at, scope, body, index): (Applying, Entity, Body, usize)) {
        let Item::Rule { selectors, body: rule_body, loc } = &body[index] else { unreachable!() };
        if unsupported_pseudo(selectors) {
            warn!("{}:{loc}: pseudo-classes other than :root are not supported yet, rule skipped", at.source.doc.path);
            return;
        }
        let matched = self.matches(&at, scope, selectors);
        let at = Applying { rule: true, ..at };
        for entity in matched {
            self.build(&at, entity, rule_body);
        }
    }
}

// CLAUDE
/// classes no rule selects by, when a rule has nearly that name
fn near_miss_classes(scene: &mut World, rule_classes: &HashSet<String>) -> Vec<String> {
    let mut used = HashSet::new();
    let mut classes = scene.query::<&Classes>();
    for classes in classes.iter(scene) {
        used.extend(classes.0.iter().cloned());
    }
    let mut warnings = vec![];
    for class in used.difference(rule_classes) {
        if let Some(near) = rule_classes.iter().find(|r| edit_distance(class, r) <= (class.len().max(r.len()) / 4).max(1)) {
            warnings.push(format!("no rule for class `.{class}`, did you mean `.{near}`?"));
        }
    }
    warnings
}

fn edit_distance(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, x) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, y) in b.chars().enumerate() {
            let next = (diagonal + usize::from(x != y)).min(row[j] + 1).min(row[j + 1] + 1);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.chars().count()]
}

fn already_spawned(world: &World, parent: Entity, spawned: Spawned) -> bool {
    let Some(children) = world.get::<Children>(parent) else { return false };
    children.iter().any(|c| world.get::<Spawned>(c) == Some(&spawned))
}

fn clone_value(value: &dyn PartialReflect, registration: &TypeRegistration) -> Box<dyn PartialReflect> {
    value.reflect_clone().map(PartialReflect::into_partial_reflect).unwrap_or_else(|_| {
        registration
            .data::<ReflectFromReflect>()
            .and_then(|f| f.from_reflect(value))
            .map(PartialReflect::into_partial_reflect)
            .unwrap_or_else(|| value.to_dynamic().unwrap())
    })
}

/// all descendants, depth first in child order
fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = vec![];
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter().rev());
        }
        if e != root {
            out.push(e);
        }
    }
    out
}

// CLAUDE
/// `root` and its descendants as a world of their own, `root` marked as a scene file root
fn copy_subtree(world: &World, root: Entity, registry: &TypeRegistry) -> World {
    let mut out = World::new();
    let mut entities = descendants(world, root);
    entities.insert(0, root);
    let mut map: EntityHashMap<Entity> = entities.iter().map(|&e| (e, out.spawn_empty().id())).collect();
    for &source in &entities {
        let target = map[&source];
        for (_, info) in world.inspect_entity(source).unwrap() {
            let Some(type_id) = info.type_id() else { continue };
            if source == root && type_id == TypeId::of::<ChildOf>() {
                continue;
            }
            let Some(registration) = registry.get(type_id) else { continue };
            let Some(component) = registration.data::<ReflectComponent>() else { continue };
            let Some(value) = component.reflect(world.entity(source)) else { continue };
            let value = clone_value(value.as_partial_reflect(), registration);
            SceneEntityMapper::world_scope(&mut map, &mut out, |out, mapper| {
                component.apply_or_insert_mapped(&mut out.entity_mut(target), value.as_ref(), registry, mapper, RelationshipHookMode::Skip);
            });
        }
    }
    out.entity_mut(map[&root]).insert(SceneFileRoot);
    out
}

// CLAUDE
/// components holding entities that a spawn wouldn't map (to the spawned entities), as warnings. A
/// component maps its entities with `#[entities]` on the fields; tested on one value of each type that
/// holds any, inserted into a scratch world through a mapper that maps every entity to a placeholder
fn check_entity_mapping(world: &World, registry: &TypeRegistry) -> Vec<String> {
    struct Placeholder;
    impl EntityMapper for Placeholder {
        fn get_mapped(&mut self, _: Entity) -> Entity {
            Entity::PLACEHOLDER
        }
        fn set_mapped(&mut self, _: Entity, _: Entity) {}
    }

    let mut warnings = vec![];
    let mut checked = HashSet::new();
    for archetype in world.archetypes().iter() {
        if archetype.contains(IS_RESOURCE) {
            continue;
        }
        for id in archetype.iter_components() {
            let Some(type_id) = world.components().get_info(id).and_then(|i| i.type_id()) else { continue };
            if checked.contains(&type_id) {
                continue;
            }
            let Some(registration) = registry.get(type_id) else { continue };
            let Some(component) = registration.data::<ReflectComponent>() else { continue };
            let value = archetype.entities().iter().find_map(|e| {
                let value = component.reflect(world.entity(e.id()))?;
                let mut found = vec![];
                entities_in(value.as_partial_reflect(), &mut found);
                (!found.is_empty()).then_some(value)
            });
            let Some(value) = value else { continue };
            checked.insert(type_id);
            let mut scratch = World::new();
            let mut target = scratch.spawn_empty();
            let value = clone_value(value.as_partial_reflect(), registration);
            component.apply_or_insert_mapped(&mut target, value.as_ref(), registry, &mut Placeholder, RelationshipHookMode::Skip);
            let mapped = component.reflect(EntityRef::from(target.into_readonly()));
            let mut after = vec![];
            if let Some(mapped) = mapped {
                entities_in(mapped.as_partial_reflect(), &mut after);
            }
            if after.iter().any(|&e| e != Entity::PLACEHOLDER) {
                warnings.push(format!(
                    "`{}` holds entities it doesn't map: once spawned they point at the wrong entities (add #[entities] to the fields)",
                    registration.type_info().type_path_table().short_path()
                ));
            }
        }
    }
    warnings
}

/// components a spawn can't copy: not registered, or not reflected as components
fn unregistered_components(world: &World, registry: &TypeRegistry) -> Vec<String> {
    let mut names = vec![];
    for archetype in world.archetypes().iter() {
        if archetype.contains(IS_RESOURCE) || archetype.is_empty() {
            continue;
        }
        for id in archetype.iter_components() {
            let Some(info) = world.components().get_info(id) else { continue };
            let reflected = info.type_id().and_then(|t| registry.get(t)).is_some_and(|r| r.contains::<ReflectComponent>());
            let name = info.name().to_string();
            if !reflected && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// every `Entity` in a value
fn entities_in(value: &dyn PartialReflect, out: &mut Vec<Entity>) {
    if let Some(entity) = value.try_downcast_ref::<Entity>() {
        out.push(*entity);
        return;
    }
    match value.reflect_ref() {
        ReflectRef::Struct(s) => s.iter_fields().for_each(|(_, f)| entities_in(f, out)),
        ReflectRef::TupleStruct(s) => s.iter_fields().for_each(|f| entities_in(f, out)),
        ReflectRef::Tuple(t) => t.iter_fields().for_each(|f| entities_in(f, out)),
        ReflectRef::List(l) => l.iter().for_each(|f| entities_in(f, out)),
        ReflectRef::Array(a) => a.iter().for_each(|f| entities_in(f, out)),
        ReflectRef::Set(s) => s.iter().for_each(|f| entities_in(f, out)),
        ReflectRef::Map(m) => m.iter().for_each(|(k, v)| {
            entities_in(k, out);
            entities_in(v, out);
        }),
        ReflectRef::Enum(e) => e.iter_fields().for_each(|f| entities_in(f.value(), out)),
        _ => {}
    }
}

fn parent(world: &World, e: Entity) -> Option<Entity> {
    world.get::<ChildOf>(e).map(|c| c.parent())
}

/// whether `e` is `scope` or one of its descendants
fn within(world: &World, scope: Entity, mut e: Entity) -> bool {
    loop {
        if e == scope {
            return true;
        }
        match parent(world, e) {
            Some(p) => e = p,
            None => return false,
        }
    }
}

fn unsupported_pseudo(selectors: &[Selector]) -> bool {
    selectors.iter().any(|s| s.parts.iter().any(|(_, c)| c.pseudo.iter().any(|p| p != "root")))
}

// CLAUDE
/// what `selectors` match in `scope`'s subtree (it included) of a spawned scene, in tree order. `#name`
/// names what `file` (a scene file's path) spawned, as in that file. `:root` is the scene file root above
/// `scope`
#[cfg(feature = "scripting")]
pub(crate) fn select_spawned(world: &World, registry: &TypeRegistry, scope: Entity, selectors: &[Selector], file: Option<&str>) -> Vec<Entity> {
    let mut root = scope;
    while world.get::<SceneFileRoot>(root).is_none() {
        match parent(world, root) {
            Some(p) => root = p,
            None => {
                root = scope;
                break;
            }
        }
    }
    let copied = HashMap::new();
    let m = Matcher { world, registry, root, build_root: root, file: file.map(file_hash), included: None, copied: &copied };
    // in tree order from the scene root (`scope` when there's none). `:root ...` may match outside `scope`
    m.select(scope, selectors)
}

// CLAUDE
/// matches selectors in the scene being built
#[derive(Clone, Copy)]
struct Matcher<'a> {
    world: &'a World,
    registry: &'a TypeRegistry,
    /// the entity the file applies to: the scope of entity references
    root: Entity,
    build_root: Entity,
    /// the file the selectors are written in (a hash of its path): `#name` names what it spawned
    file: Option<u64>,
    /// see [`Applying::included`]
    included: Option<u64>,
    copied: &'a HashMap<Entity, u64>,
}

impl Matcher<'_> {
    /// the entities in `scope` (it and its descendants) any of `selectors` matches, in tree order. a selector
    /// starting at `:root` selects from the root of the scene
    fn select(&self, scope: Entity, selectors: &[Selector]) -> Vec<Entity> {
        let mut found = HashSet::new();
        for s in selectors {
            let scope = match s.parts[0].1.pseudo.iter().any(|p| p == "root") {
                true => self.build_root,
                false => scope,
            };
            if self.world.get_entity(scope).is_err() {
                continue;
            }
            found.extend(std::iter::once(scope).chain(descendants(self.world, scope)).filter(|&e| self.selector_matches(scope, s, s.parts.len() - 1, e)));
        }
        std::iter::once(self.build_root).chain(descendants(self.world, self.build_root)).filter(|e| found.contains(e)).collect()
    }

    // CLAUDE
    /// whether `#name` can name `e`: names are a file's own. `e` was spawned by the selectors' file, or, in
    /// an include's body, by the file included or copied from the scene included. a [`Spawned`] carries its
    /// file (scripts' too, while a file builds)
    fn names(&self, e: Entity) -> bool {
        let file = self.world.get::<Spawned>(e).map(|s| s.file).or_else(|| self.copied.get(&e).copied());
        file.is_some() && (file == self.file || file == self.included)
    }

    /// the entities `component` relates `e` to, for `/Comp/` before `e`: the ones whose `component` holds
    /// `e`, or for a relationship (`ChildOf`, `ListenerOf`) the ones `e`'s holds
    fn related(&self, component: TypeId, e: Entity) -> Vec<Entity> {
        let world = self.world;
        let Some(id) = world.components().get_id(component) else { return vec![] };
        let Some(reflect) = self.registry.get_type_data::<ReflectComponent>(component) else { return vec![] };
        let held = |entity: EntityRef| {
            let mut out = vec![];
            if let Some(value) = reflect.reflect(entity) {
                entities_in(value.as_partial_reflect(), &mut out);
            }
            out
        };
        let relationship = world
            .components()
            .get_info(id)
            .and_then(|info| info.relationship_accessor())
            .is_some_and(|a| matches!(a, RelationshipAccessor::Relationship { .. }));
        if relationship {
            return held(world.entity(e));
        }
        world.iter_entities().filter(|entity| entity.contains_id(id) && held(*entity).contains(&e)).map(|entity| entity.id()).collect()
    }

    /// what each entity reference in `value` matches, in the file's scope
    fn refs(&self, value: &Value) -> EntityRefs {
        let mut refs = EntityRefs::default();
        value.each_ref(&mut |v| match v {
            Value::Hex(name) if !refs.names.contains_key(name) => {
                let by_name = Selector { parts: vec![(Combinator::Descendant, Compound { name: Some(name.clone()), ..default() })] };
                refs.names.insert(name.clone(), self.select(self.root, &[by_name]));
            }
            Value::Select(list) => {
                refs.selects.insert(Arc::as_ptr(&list.0) as usize, self.select(self.root, &list.0));
            }
            _ => {}
        });
        refs
    }

    /// whether `selector.parts[..=i]` matches with part `i` on `e`, inside `scope`
    fn selector_matches(&self, scope: Entity, selector: &Selector, i: usize, e: Entity) -> bool {
        let world = self.world;
        let (combinator, compound) = &selector.parts[i];
        if !self.compound_matches(compound, e) {
            return false;
        }
        if let Combinator::Relation(component) = combinator {
            let related = self.related(*component, e);
            if i == 0 {
                return related.contains(&scope);
            }
            return related.into_iter().any(|r| within(world, scope, r) && self.selector_matches(scope, selector, i - 1, r));
        }
        if i == 0 {
            return match combinator {
                Combinator::Child => parent(world, e) == Some(scope),
                // `e` is the scope, or came from its descendants
                _ => true,
            };
        }
        if e == scope {
            return false;
        }
        let Some(mut up) = parent(world, e) else { return false };
        match combinator {
            Combinator::Child => self.selector_matches(scope, selector, i - 1, up),
            _ => loop {
                if self.selector_matches(scope, selector, i - 1, up) {
                    return true;
                }
                if up == scope {
                    return false;
                }
                match parent(world, up) {
                    Some(p) => up = p,
                    None => return false,
                }
            },
        }
    }

    fn compound_matches(&self, c: &Compound, e: Entity) -> bool {
        let registry = self.registry;
        let entity = self.world.entity(e);
        if !c.components.iter().all(|&t| entity.contains_type_id(t)) {
            return false;
        }
        // only `:root` is supported
        if !c.pseudo.iter().all(|p| p == "root" && e == self.build_root) {
            return false;
        }
        if let Some(name) = &c.name {
            if entity.get::<Name>().is_none_or(|n| n.as_str() != name) || !self.names(e) {
                return false;
            }
        }
        if !c.classes.is_empty() {
            let Some(classes) = entity.get::<Classes>() else { return false };
            if !c.classes.iter().all(|class| classes.0.contains(class)) {
                return false;
            }
        }
        c.attrs.iter().all(|attr| {
            let Some(value) = component_value(registry, entity, attr.component) else { return false };
            let Some(value) = field_at(value, &attr.field) else { return false };
            let Some((op, expected)) = &attr.test else { return true };
            if *op == AttrOp::Equals {
                return self.equals(value, expected);
            }
            // the parser allows only text for the others
            let (Some(text), Some(expected)) = (value_text(registry, value), expected.as_text()) else { return false };
            match op {
                AttrOp::Word => text.split_whitespace().any(|w| w == expected),
                AttrOp::Contains => text.contains(expected.as_str()),
                AttrOp::Prefix => text.starts_with(expected.as_str()),
                AttrOp::Suffix => text.ends_with(expected.as_str()),
                AttrOp::Equals => unreachable!(),
            }
        })
    }

    // CLAUDE
    /// `[Comp.field=expected]`: an entity among what a reference matches; text for strings, names, numbers,
    /// unit variants and handles; else equal to the field's type patched with `expected`
    fn equals(&self, value: &dyn PartialReflect, expected: &Value) -> bool {
        // `ChildOf` and the like: their entity
        let mut inner = value;
        while let ReflectRef::TupleStruct(s) = inner.reflect_ref() {
            match s.field(0) {
                Some(field) if s.field_len() == 1 => inner = field,
                _ => break,
            }
        }
        if let Some(entity) = inner.try_downcast_ref::<Entity>() {
            return self.refs(expected).get(expected).is_some_and(|found| found.contains(entity));
        }
        if let (Some(text), Some(expected)) = (value_text(self.registry, value), expected.as_text()) {
            return text == expected;
        }
        let Some(info) = value.get_represented_type_info() else { return false };
        let Ok(mut parsed) = default_of(info, self.registry) else { return false };
        let refs = self.refs(expected);
        let cx = ParseCx { registry: self.registry, assets: None, named: None, json: None, entities: Some(&refs) };
        if patch(parsed.as_mut(), expected, &cx).is_err() {
            return false;
        }
        // a type's own PartialEq doesn't take a dynamic value
        if let Some(concrete) = self.registry.get_type_data::<ReflectFromReflect>(info.type_id()).and_then(|f| f.from_reflect(parsed.as_ref())) {
            parsed = concrete.into_partial_reflect();
        }
        value.reflect_partial_eq(parsed.as_ref()) == Some(true)
    }
}

fn component_value<'w>(registry: &TypeRegistry, entity: EntityRef<'w>, component: TypeId) -> Option<&'w dyn PartialReflect> {
    let reflect = registry.get_type_data::<ReflectComponent>(component)?;
    reflect.reflect(entity).map(|r| r.as_partial_reflect())
}

// CLAUDE
/// sets the field at `path` of `to` from `from`'s; false if either hasn't the field or it doesn't apply
fn copy_field(from: &dyn PartialReflect, to: &mut dyn PartialReflect, path: &[String]) -> bool {
    let Some(source) = field_at(from, path) else { return false };
    let mut target = to;
    for field in path {
        let next = match target.reflect_mut() {
            ReflectMut::Struct(s) => s.field_mut(field),
            ReflectMut::TupleStruct(s) => field.parse().ok().and_then(|i| s.field_mut(i)),
            ReflectMut::Tuple(t) => field.parse().ok().and_then(|i| t.field_mut(i)),
            ReflectMut::Enum(e) => match field.parse() {
                Ok(i) => e.field_at_mut(i),
                Err(_) => e.field_mut(field),
            },
            _ => None,
        };
        let Some(next) = next else { return false };
        target = next;
    }
    target.try_apply(source).is_ok()
}

fn field_at<'a>(mut value: &'a dyn PartialReflect, path: &[String]) -> Option<&'a dyn PartialReflect> {
    for field in path {
        value = match value.reflect_ref() {
            ReflectRef::Struct(s) => s.field(field)?,
            ReflectRef::TupleStruct(s) => s.field(field.parse().ok()?)?,
            ReflectRef::Enum(e) => e.field(field)?,
            _ => return None,
        };
    }
    Some(value)
}

/// a value as text for attribute tests: strings, names, numbers, bools, unit variants, and
/// asset handles (their path), one-field wrappers of those
fn value_text(registry: &TypeRegistry, value: &dyn PartialReflect) -> Option<String> {
    if let Some(key) = corn_assets::handle_key(value, registry) {
        return Some(key);
    }
    if let Some(s) = value.try_downcast_ref::<String>() {
        return Some(s.clone());
    }
    if let Some(n) = value.try_downcast_ref::<Name>() {
        return Some(n.to_string());
    }
    match value.reflect_ref() {
        ReflectRef::TupleStruct(s) if s.field_len() == 1 => value_text(registry, s.field(0)?),
        // an Option's value
        ReflectRef::Enum(e) if e.variant_name() == "Some" && e.field_len() == 1 && value.reflect_type_path().starts_with("core::option::Option<") => {
            value_text(registry, e.field_at(0)?)
        }
        // for `~=`
        ReflectRef::List(l) => Some(l.iter().filter_map(|v| value_text(registry, v)).collect::<Vec<_>>().join(" ")),
        ReflectRef::Enum(e) if e.field_len() == 0 => Some(e.variant_name().to_string()),
        ReflectRef::Opaque(_) => {
            macro_rules! show {
                ($($t:ty),*) => {$( if let Some(v) = value.try_downcast_ref::<$t>() { return Some(v.to_string()); } )*};
            }
            show!(f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize, bool);
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    struct Target(Entity);

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    struct Mapped(#[entities] Entity);

    #[test]
    fn near_misses() {
        let mut world = World::new();
        world.spawn(Classes(vec!["static-bx".into(), "unrelated".into(), "fine".into()]));
        let rules = HashSet::from(["static-box".to_string(), "fine".to_string()]);
        assert_eq!(near_miss_classes(&mut world, &rules), ["no rule for class `.static-bx`, did you mean `.static-box`?"]);
    }

    #[test]
    fn ranks() {
        let rank = |layer: &[u32], direct| Rank { layer: Arc::new(layer.to_vec()), direct };
        // a file over what it includes, whatever the kind of write
        assert!(rank(&[], false).beats(&rank(&[1], true)));
        // the later include, with what it includes
        assert!(rank(&[2], false).beats(&rank(&[1], true)));
        assert!(rank(&[2, 3], false).beats(&rank(&[1], true)));
        // direct over a rule; a tie doesn't keep the later write out
        assert!(rank(&[1], true).beats(&rank(&[1], false)));
        assert!(!rank(&[1], true).beats(&rank(&[1], true)));
    }

    #[test]
    fn warns_on_unmapped_entities() {
        let mut registry = TypeRegistry::new();
        registry.register::<Target>();
        registry.register::<Mapped>();
        registry.register::<ChildOf>();
        registry.register::<Children>();
        let mut world = World::new();
        let a = world.spawn_empty().id();
        world.spawn((Target(a), Mapped(a), ChildOf(a)));
        let warnings = check_entity_mapping(&world, &registry);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("`Target`"), "{warnings:?}");
    }
}
