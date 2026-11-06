//! CLAUDE (claude-opus-5-5): live references into components, resources and assets, shared by every engine.
//!
//! A [`LiveRef`] is (base, field path), the base being an entity's component, a resource or an asset.
//! Each language wraps it in its own handle type, and reads or writes through [`read`] and [`write`].
//!
//! Resolving a path to a pointer goes through reflection once; the pointer is cached until anything
//! could move the memory: the end of a script call (scripts `yield` between calls), a structural
//! host call (insert, remove, spawn, despawn), or a write that isn't a plain leaf (it may change an
//! enum variant or a list's length).
//!
//! A ref also keeps its own pointer, and a child made from a resolved ref is resolved by one reflect
//! step, so walking down a component (`t.translation.x`) does no lookups. Paths are interned ids.
//!
//! Assets are read through a shared pointer until the first write: bevy can only hand out a mutable
//! asset by marking it modified, and reading a material must not re-upload it.

use std::{
    any::TypeId,
    cell::{Cell, RefCell},
};

use bevy::{
    asset::{ReflectAsset, UntypedAssetId},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    reflect::{enums::VariantType, serde::TypedReflectSerializer, ReflectMut, ReflectRef, TypeRegistry},
};

use super::{
    apply_json, entity_key,
    math::{Kind, Math},
    parse_entity,
    values::{self, ScriptValues},
    with_host_world,
};

// CLAUDE
#[derive(Debug, Clone)]
pub struct LiveRef {
    base: Base,
    path: PathId,
    /// (cache generation, pointer): valid while the generation is current
    resolved: Cell<Option<(u64, *mut dyn PartialReflect)>>,
}

// CLAUDE
/// what a [`LiveRef`] points into
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Base {
    Component(Entity, TypeId),
    Resource(TypeId),
    Asset(UntypedAssetId),
}

impl Base {
    pub fn type_id(&self) -> TypeId {
        match self {
            Base::Component(_, t) | Base::Resource(t) => *t,
            Base::Asset(id) => id.type_id(),
        }
    }

    /// whether pointers into it may be written through. see the module docs
    fn writable(&self) -> bool {
        !matches!(self, Base::Asset(_)) || CHANGED.with_borrow(|c| c.contains(self))
    }
}

/// an interned field path, see [`intern`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PathId(u32);

/// the component itself
const ROOT: PathId = PathId(0);

// CLAUDE
struct Paths {
    /// by id: (parent, last key). the root's entry is unused
    nodes: Vec<(PathId, Box<str>)>,
    children: HashMap<PathId, HashMap<Box<str>, PathId>>,
}

/// the path `key` under `parent`. paths are never forgotten: there are only as many as scripts name
fn intern(parent: PathId, key: &str) -> PathId {
    PATHS.with_borrow_mut(|p| {
        if let Some(id) = p.children.get(&parent).and_then(|c| c.get(key)) {
            return *id;
        }
        let id = PathId(p.nodes.len() as u32);
        p.nodes.push((parent, key.into()));
        p.children.entry(parent).or_default().insert(key.into(), id);
        id
    })
}

/// `a.b.c`, for messages
fn path_string(mut path: PathId) -> String {
    PATHS.with_borrow(|p| {
        let mut keys = Vec::new();
        while path != ROOT {
            let (parent, key) = &p.nodes[path.0 as usize];
            keys.push(&**key);
            path = *parent;
        }
        keys.reverse();
        keys.join(".")
    })
}

/// a value read from, or written to, a live reference
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// anything else, as reflect-serialized data
    Json(serde_json::Value),
    /// a math value, see [`super::math`]
    Math(Math),
}

pub enum Read {
    Value(Field),
    /// a struct, enum, list...: read further through this ref
    Ref(LiveRef),
    /// a math type: the engine's math value, living here
    Math(Kind, LiveRef),
    Missing,
}

impl LiveRef {
    pub fn child(&self, key: &str) -> LiveRef {
        let child = LiveRef { base: self.base, path: intern(self.path, key), resolved: Cell::new(None) };
        if let Some(parent) = self.cached() {
            if let Some(field) = step_ptr(parent, key, self.base.writable()) {
                child.remember(field);
            }
        }
        child
    }

    /// the type of the component, resource or asset
    pub fn root_type(&self) -> TypeId {
        self.base.type_id()
    }

    /// the pointer, if resolved since the cache was last cleared
    fn cached(&self) -> Option<*mut dyn PartialReflect> {
        self.resolved.get().filter(|(generation, _)| *generation == GENERATION.get()).map(|(_, ptr)| ptr)
    }

    fn remember(&self, ptr: *mut dyn PartialReflect) {
        self.resolved.set(Some((GENERATION.get(), ptr)));
    }

    fn changed(&self) -> bool {
        CHANGED.with_borrow(|c| c.contains(&self.base))
    }
}

thread_local! {
    static PATHS: RefCell<Paths> = RefCell::new(Paths { nodes: vec![(ROOT, "".into())], children: HashMap::default() });
    static CACHE: RefCell<HashMap<(Base, PathId), *mut dyn PartialReflect>> = RefCell::default();
    /// bases marked changed since the cache was last cleared
    static CHANGED: RefCell<HashSet<Base>> = RefCell::default();
    /// bumped by every clear: pointers kept in refs are only good for their generation
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

/// forget resolved pointers. see the module docs for when
pub fn clear_cache() {
    CHANGED.with_borrow_mut(HashSet::clear);
    forget_pointers();
}

fn forget_pointers() {
    CACHE.with_borrow_mut(HashMap::clear);
    GENERATION.set(GENERATION.get() + 1);
}

// CLAUDE
/// a live reference to an entity's component
pub fn component(entity: &str, type_name: &str) -> Result<LiveRef, String> {
    let entity = parse_entity(entity)?;
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        let (registration, component) = super::lookup(&registry, type_name)?;
        let entity_ref = world.get_entity(entity).map_err(|e| e.to_string())?;
        if !component.contains(entity_ref) {
            return Err(format!("entity {} has no `{type_name}`", entity_key(entity)));
        }
        if !super::is_mutable(world, component) {
            return Err(format!("`{type_name}` is immutable: replace it with insert"));
        }
        let r = LiveRef { base: Base::Component(entity, registration.type_id()), path: ROOT, resolved: Cell::new(None) };
        // resolved now, so its children resolve as they're made
        resolve(world, &registry, &r)?;
        Ok(r)
    })
}

// CLAUDE
/// a live reference to a resource, None if it doesn't exist
pub fn resource(type_name: &str) -> Result<Option<LiveRef>, String> {
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        let registration = super::registration(&registry, type_name)?;
        if !registration.contains::<ReflectResource>() {
            return Err(format!("`{type_name}` is not a reflected resource"));
        }
        let r = LiveRef { base: Base::Resource(registration.type_id()), path: ROOT, resolved: Cell::new(None) };
        Ok(resolve(world, &registry, &r).is_ok().then_some(r))
    })
}

// CLAUDE
/// a live reference to an asset, by its path or the id a path-less handle reads as (see [`values`]).
/// None if it isn't loaded
pub fn asset(key: &str, type_name: &str) -> Result<Option<LiveRef>, String> {
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        let registration = super::registration(&registry, type_name)?;
        if !registration.contains::<ReflectAsset>() {
            return Err(format!("`{type_name}` is not a reflected asset (register it with `register_asset_reflect`)"));
        }
        let type_id = registration.type_id();
        let id = match values::parse_asset_id(key, type_id) {
            Some(id) => Some(id),
            None => super::asset_server(world).and_then(|assets| {
                assets.get_path_ids(key.to_string()).into_iter().find(|id| id.type_id() == type_id)
            }),
        };
        let Some(id) = id else {
            return Ok(None);
        };
        let r = LiveRef { base: Base::Asset(id), path: ROOT, resolved: Cell::new(None) };
        Ok(resolve(world, &registry, &r).is_ok().then_some(r))
    })
}

// CLAUDE
pub fn read(r: &LiveRef) -> Result<Read, String> {
    if let Some(ptr) = r.cached() {
        // SAFETY: see `resolve`
        if let Some(read) = read_plain(r, unsafe { &*ptr }) {
            return Ok(read);
        }
    }
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        let Some(ptr) = resolve(world, &registry, r)? else {
            return Ok(Read::Missing);
        };
        // SAFETY: see `resolve`
        let value = unsafe { &*ptr };
        if let Some(read) = read_plain(r, value) {
            return Ok(read);
        }
        Ok(match leaf(value, &registry)? {
            Some(field) => Read::Value(field),
            None => Read::Ref(r.clone()),
        })
    })
}

/// a read that needs no type registry: math values and plain leaves
fn read_plain(r: &LiveRef, value: &dyn PartialReflect) -> Option<Read> {
    if let Some(kind) = value.get_represented_type_info().and_then(|info| Kind::of(info.type_id())) {
        return Some(Read::Math(kind, r.clone()));
    }
    plain_leaf(value).map(Read::Value)
}

// CLAUDE
pub fn write(r: &LiveRef, value: Field) -> Result<(), String> {
    // CLAUDE (claude-sonnet-5-5): a NaN or infinity written to any float field (`Player.yaw`...) reaches bevy's
    // math later and panics there; math values must also be finite, with rotations normalized
    let value = match value {
        Field::Float(f) if !f.is_finite() => return Err(format!("a number must be finite, not {f}")),
        Field::Math(math) => Field::Math(math.sanitized()?),
        value => value,
    };
    if let Some(ptr) = r.cached() {
        // SAFETY: see `resolve`
        if r.changed() && set_leaf(unsafe { &mut *ptr }, &value)? {
            return Ok(());
        }
    }
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        // first: an asset only gives a writable pointer once marked
        mark_changed(world, &registry, r);
        let ptr = resolve(world, &registry, r)?.ok_or_else(|| format!("no field `{}`", path_string(r.path)))?;
        // SAFETY: see `resolve`. no other reference into the component is alive
        let target = unsafe { &mut *ptr };
        if set_leaf(target, &value)? {
            return Ok(());
        }
        let json = value.into_json();
        let assets = super::asset_server(world);
        clear_cache();
        apply_json(target, &json, &registry, assets.as_ref())
    })
}

/// the pointer for this ref's path, None if a field is missing. Err if the entity or component is gone.
///
/// The pointer stays valid until [`clear_cache`]: scripts run with exclusive access to the world, and
/// everything that could move or free component memory clears the cache first.
fn resolve(world: &mut World, registry: &TypeRegistry, r: &LiveRef) -> Result<Option<*mut dyn PartialReflect>, String> {
    if let Some(ptr) = r.cached() {
        return Ok(Some(ptr));
    }
    let key = (r.base, r.path);
    if let Some(ptr) = CACHE.with_borrow(|c| c.get(&key).copied()) {
        r.remember(ptr);
        return Ok(Some(ptr));
    }

    let ptr = if r.path == ROOT {
        root(world, registry, r.base)?
    } else {
        let (parent, field) = PATHS.with_borrow(|p| p.nodes[r.path.0 as usize].clone());
        let parent = LiveRef { path: parent, resolved: Cell::new(None), ..r.clone() };
        let Some(parent) = resolve(world, registry, &parent)? else {
            return Ok(None);
        };
        match step_ptr(parent, &field, r.base.writable()) {
            Some(field) => field,
            None => return Ok(None),
        }
    };
    CACHE.with_borrow_mut(|c| c.insert(key, ptr));
    r.remember(ptr);
    Ok(Some(ptr))
}

// CLAUDE
/// the pointer to the component, resource or asset itself. reading must not mark it changed, see `mark_changed`
fn root(world: &mut World, registry: &TypeRegistry, base: Base) -> Result<*mut dyn PartialReflect, String> {
    Ok(match base {
        Base::Component(entity, type_id) => {
            let component = registry.get_type_data::<ReflectComponent>(type_id).ok_or("not a reflected component")?;
            let mut entity = world.get_entity_mut(entity).map_err(|e| e.to_string())?;
            let mut value = component.reflect_mut(&mut entity).ok_or("the entity no longer has this component")?;
            value.bypass_change_detection().as_partial_reflect_mut() as *mut dyn PartialReflect
        }
        Base::Resource(type_id) => {
            let component = registry.get_type_data::<ReflectComponent>(type_id).ok_or("not a reflected resource")?;
            let entity = super::resource_entity(world, type_id).ok_or("the resource no longer exists")?;
            let mut entity = world.entity_mut(entity);
            let mut value = component.reflect_mut(&mut entity).ok_or("the resource no longer exists")?;
            value.bypass_change_detection().as_partial_reflect_mut() as *mut dyn PartialReflect
        }
        Base::Asset(id) => {
            let asset = registry.get_type_data::<ReflectAsset>(id.type_id()).ok_or("not a reflected asset")?;
            let gone = || "the asset no longer exists".to_string();
            if base.writable() {
                // marks it modified
                asset.get_mut(world, id).ok_or_else(gone)?.as_partial_reflect_mut() as *mut dyn PartialReflect
            } else {
                // only ever read through, see `step_ptr`
                let value = asset.get(world, id).ok_or_else(gone)?.as_partial_reflect();
                value as *const dyn PartialReflect as *mut dyn PartialReflect
            }
        }
    })
}

/// one field down from a resolved pointer. a pointer that isn't `writable` is only ever borrowed shared
fn step_ptr(ptr: *mut dyn PartialReflect, key: &str, writable: bool) -> Option<*mut dyn PartialReflect> {
    // SAFETY: see `resolve`
    if writable {
        step(unsafe { &mut *ptr }, key).map(|field| field as *mut dyn PartialReflect)
    } else {
        step_ref(unsafe { &*ptr }, key).map(|field| field as *const dyn PartialReflect as *mut dyn PartialReflect)
    }
}

// CLAUDE
/// run `f` on the value at `r`, in place
pub fn with_value<R>(r: &LiveRef, f: impl FnOnce(&dyn PartialReflect) -> R) -> Result<R, String> {
    if let Some(ptr) = r.cached() {
        // SAFETY: see `resolve`
        return Ok(f(unsafe { &*ptr }));
    }
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        let ptr = resolve(world, &registry, r)?.ok_or_else(|| format!("no field `{}`", path_string(r.path)))?;
        // SAFETY: see `resolve`
        Ok(f(unsafe { &*ptr }))
    })
}

// CLAUDE
/// run `f` on the value at `r`, in place, marking it changed
pub fn with_value_mut<R>(r: &LiveRef, f: impl FnOnce(&mut dyn PartialReflect) -> R) -> Result<R, String> {
    if let Some(ptr) = r.cached().filter(|_| r.changed()) {
        // SAFETY: see `resolve`. changed, so writable
        return Ok(f(unsafe { &mut *ptr }));
    }
    with_host_world(|world| {
        let registry = super::type_registry(world);
        let registry = registry.read();
        mark_changed(world, &registry, r);
        let ptr = resolve(world, &registry, r)?.ok_or_else(|| format!("no field `{}`", path_string(r.path)))?;
        // SAFETY: see `resolve`
        Ok(f(unsafe { &mut *ptr }))
    })
}

/// a path (as in a [`LiveRef`]) into a value
pub fn walk<'a>(value: &'a mut dyn PartialReflect, path: &str) -> Option<&'a mut dyn PartialReflect> {
    path.split('.').filter(|key| !key.is_empty()).try_fold(value, step)
}

/// one field down: a name, an index into tuples, lists and enum variants, or a string-keyed map's key
fn step<'a>(value: &'a mut dyn PartialReflect, key: &str) -> Option<&'a mut dyn PartialReflect> {
    let index = key.parse::<usize>().ok();
    match value.reflect_mut() {
        ReflectMut::Struct(s) => s.field_mut(key),
        ReflectMut::TupleStruct(s) => s.field_mut(index?),
        ReflectMut::Tuple(t) => t.field_mut(index?),
        ReflectMut::Enum(e) => match index {
            Some(i) => e.field_at_mut(i),
            None => e.field_mut(key),
        },
        ReflectMut::List(l) => l.get_mut(index?),
        ReflectMut::Array(a) => a.get_mut(index?),
        ReflectMut::Map(m) => m.get_mut(&key.to_string()),
        _ => None,
    }
}

/// `step`, shared
fn step_ref<'a>(value: &'a dyn PartialReflect, key: &str) -> Option<&'a dyn PartialReflect> {
    let index = key.parse::<usize>().ok();
    match value.reflect_ref() {
        ReflectRef::Struct(s) => s.field(key),
        ReflectRef::TupleStruct(s) => s.field(index?),
        ReflectRef::Tuple(t) => t.field(index?),
        ReflectRef::Enum(e) => match index {
            Some(i) => e.field_at(i),
            None => e.field(key),
        },
        ReflectRef::List(l) => l.get(index?),
        ReflectRef::Array(a) => a.get(index?),
        ReflectRef::Map(m) => m.get(&key.to_string()),
        _ => None,
    }
}

fn mark_changed(world: &mut World, registry: &TypeRegistry, r: &LiveRef) {
    if !CHANGED.with_borrow_mut(|c| c.insert(r.base)) {
        return;
    }
    match r.base {
        Base::Component(entity, type_id) => {
            let component = registry.get_type_data::<ReflectComponent>(type_id);
            if let (Some(component), Ok(mut entity)) = (component, world.get_entity_mut(entity)) {
                if let Some(mut value) = component.reflect_mut(&mut entity) {
                    value.set_changed();
                }
            }
        }
        Base::Resource(type_id) => {
            let component = registry.get_type_data::<ReflectComponent>(type_id);
            if let (Some(component), Some(entity)) = (component, super::resource_entity(world, type_id)) {
                if let Some(mut value) = component.reflect_mut(&mut world.entity_mut(entity)) {
                    value.set_changed();
                }
            }
        }
        // its pointers so far are shared: the next resolve takes a writable one, which marks it modified
        Base::Asset(_) => forget_pointers(),
    }
}

macro_rules! numbers {
    ($value:ident, $float:ident, $int:ident) => {
        numbers!(@float $value $float f32 f64);
        numbers!(@int $value $int i8 i16 i32 i64 isize u8 u16 u32 u64 usize);
    };
    (@float $value:ident $f:ident $($t:ty)*) => { $(if let Some($f) = $value.try_downcast_ref::<$t>() { return Some(Field::Float(*$f as f64)); })* };
    (@int $value:ident $i:ident $($t:ty)*) => { $(if let Some($i) = $value.try_downcast_ref::<$t>() { return Some(Field::Int(*$i as i64)); })* };
}

/// numbers, bools, strings, entities and enums in a variant without fields
fn plain_leaf(value: &dyn PartialReflect) -> Option<Field> {
    numbers!(value, float, int);
    if let Some(b) = value.try_downcast_ref::<bool>() {
        return Some(Field::Bool(*b));
    }
    if let Some(s) = value.try_downcast_ref::<String>() {
        return Some(Field::Str(s.clone()));
    }
    if let Some(e) = value.try_downcast_ref::<Entity>() {
        return Some(Field::Str(entity_key(*e)));
    }
    // its variant's name, as `get` gives it (None is null there too). a variant with fields is read into
    if let ReflectRef::Enum(e) = value.reflect_ref() {
        if e.variant_type() == VariantType::Unit {
            let option = e.get_represented_type_info().is_some_and(|info| {
                let path = info.type_path_table();
                path.module_path() == Some("core::option") && path.ident() == Some("Option")
            });
            return Some(if option { Field::Null } else { Field::Str(e.variant_name().into()) });
        }
    }
    None
}

/// a plain value, or None for something to read further into
fn leaf(value: &dyn PartialReflect, registry: &TypeRegistry) -> Result<Option<Field>, String> {
    if let Some(field) = plain_leaf(value) {
        return Ok(Some(field));
    }
    if super::values::is_handle(value, registry) || matches!(value.reflect_ref(), ReflectRef::Opaque(_)) {
        let processor = ScriptValues(None);
        let json = serde_json::to_value(TypedReflectSerializer::with_processor(value, registry, &processor))
            .map_err(|e| e.to_string())?;
        return Ok(Some(Field::Json(json)));
    }
    Ok(None)
}

macro_rules! set_numbers {
    ($target:ident, $value:ident; $($t:ty)*) => {
        $(if let Some(target) = $target.try_downcast_mut::<$t>() {
            *target = match $value {
                Field::Float(f) => *f as $t,
                Field::Int(i) => *i as $t,
                other => return Err(format!("expected a number, got {other:?}")),
            };
            return Ok(true);
        })*
    };
}

/// write a plain value into a plain field. false if the target isn't one
fn set_leaf(target: &mut dyn PartialReflect, value: &Field) -> Result<bool, String> {
    if let Field::Math(math) = value {
        target.try_apply(math.as_reflect()).map_err(|e| e.to_string())?;
        return Ok(true);
    }
    set_numbers!(target, value; f32 f64 i8 i16 i32 i64 isize u8 u16 u32 u64 usize);
    if let Some(target) = target.try_downcast_mut::<bool>() {
        let Field::Bool(b) = value else { return Err(format!("expected a bool, got {value:?}")) };
        *target = *b;
        return Ok(true);
    }
    if let Some(target) = target.try_downcast_mut::<String>() {
        let Field::Str(s) = value else { return Err(format!("expected a string, got {value:?}")) };
        target.clone_from(s);
        return Ok(true);
    }
    if let Some(target) = target.try_downcast_mut::<Entity>() {
        let Field::Str(s) = value else { return Err(format!("expected an entity, got {value:?}")) };
        *target = parse_entity(s)?;
        return Ok(true);
    }
    Ok(false)
}

impl Field {
    pub fn into_json(self) -> serde_json::Value {
        match self {
            Field::Null => serde_json::Value::Null,
            Field::Bool(b) => b.into(),
            Field::Int(i) => i.into(),
            Field::Float(f) => f.into(),
            Field::Str(s) => s.into(),
            Field::Json(json) => json,
            Field::Math(math) => math.to_json(),
        }
    }

    pub fn from_json(json: serde_json::Value) -> Field {
        match json {
            serde_json::Value::Null => Field::Null,
            serde_json::Value::Bool(b) => Field::Bool(b),
            serde_json::Value::Number(n) => match n.as_i64() {
                Some(i) => Field::Int(i),
                None => Field::Float(n.as_f64().unwrap_or(f64::NAN)),
            },
            serde_json::Value::String(s) => Field::Str(s),
            json => Field::Json(json),
        }
    }
}

// CLAUDE
/// the web glue's ref api: JSON in and out, the ref as (base, type, path). the base is `c:entity`
/// for a component, `r:` for a resource, `a:key` for an asset (see [`asset`])
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub mod json {
    use super::*;

    /// the root ref, None if a resource or asset is missing
    pub fn root(base: &str, type_name: &str) -> Result<Option<LiveRef>, String> {
        match base.split_once(':') {
            Some(("c", entity)) => component(entity, type_name).map(Some),
            Some(("r", _)) => resource(type_name),
            Some(("a", key)) => asset(key, type_name),
            _ => Err(format!("not a ref base: `{base}`")),
        }
    }

    pub fn at(base: &str, type_name: &str, path: &str) -> Result<LiveRef, String> {
        at_or_missing(base, type_name, path)?.ok_or_else(|| format!("`{type_name}` is gone"))
    }

    fn at_or_missing(base: &str, type_name: &str, path: &str) -> Result<Option<LiveRef>, String> {
        let root = root(base, type_name)?;
        Ok(root.map(|root| path.split('.').filter(|key| !key.is_empty()).fold(root, |r, key| r.child(key))))
    }

    /// `{"value": v}`, `{"ref": true}`, `{"math": kind}`, or `{}` if missing
    pub fn get(base: &str, type_name: &str, path: &str) -> Result<String, String> {
        let Some(r) = at_or_missing(base, type_name, path)? else {
            return Ok("{}".into());
        };
        Ok(match read(&r)? {
            Read::Value(field) => serde_json::json!({ "value": field.into_json() }),
            Read::Ref(_) => serde_json::json!({ "ref": true }),
            Read::Math(kind, _) => serde_json::json!({ "math": kind.name() }),
            Read::Missing => serde_json::json!({}),
        }
        .to_string())
    }

    pub fn set(base: &str, type_name: &str, path: &str, json: &str) -> Result<(), String> {
        let value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        write(&at(base, type_name, path)?, Field::from_json(value))
    }
}

// CLAUDE
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{host_insert, with_world_scope};

    /// shaped like bevy's DistanceFog
    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct Fog {
        color: f32,
        falloff: Falloff,
    }

    #[derive(Reflect)]
    enum Falloff {
        Linear { start: f32, end: f32 },
        Exponential { density: f32 },
    }

    impl Default for Falloff {
        fn default() -> Self {
            Falloff::Linear { start: 0.0, end: 100.0 }
        }
    }

    fn cached() -> usize {
        CACHE.with_borrow(|c| c.len())
    }

    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn resolution_is_cached_until_structural_change_or_scope_end() {
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        world.resource::<AppTypeRegistry>().write().register::<Fog>();
        let camera = entity_key(world.spawn(Fog::default()).id());

        with_world_scope(&mut world, || {
            let fog = component(&camera, "Fog").unwrap();
            let start = fog.child("falloff").child("start");
            // the component resolved through the cache, its children by stepping down from it
            assert_eq!(cached(), 1);
            let ptr = start.cached().expect("a child of a resolved ref is resolved");
            assert!(matches!(read(&start), Ok(Read::Value(Field::Float(0.0)))));
            assert!(std::ptr::addr_eq(ptr, start.cached().unwrap()), "a read reuses the pointer");
            write(&start, Field::Float(4.0)).unwrap();
            assert!(start.cached().is_some(), "leaf writes keep the cache");
            let again = component(&camera, "Fog").unwrap();
            assert_eq!(cached(), 1, "a new ref to the same component reuses its resolution");
            assert!(matches!(read(&again.child("falloff").child("start")), Ok(Read::Value(Field::Float(4.0)))));
            write(&fog.child("falloff"), Field::Json(serde_json::json!({ "Exponential": { "density": 1.0 } }))).unwrap();
            assert_eq!(cached(), 0, "a write that can change layout clears it");
            assert!(start.cached().is_none(), "and the pointers refs keep");
            assert!(matches!(read(&start), Ok(Read::Missing)), "the variant has no `start` now");
            assert!(cached() > 0);
            host_insert(&camera, "Fog", "{}").unwrap();
            assert_eq!(cached(), 0, "structural host calls clear it");
            read(&fog.child("color")).unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(cached(), 0, "nothing survives the call");
    }

    // CLAUDE (claude-sonnet-5-5)
    /// a script can't write NaN, infinity or a non-unit rotation into a component
    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn non_finite_and_non_unit_values_are_refused() {
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        world.resource::<AppTypeRegistry>().write().register::<Fog>();
        world.resource::<AppTypeRegistry>().write().register::<Transform>();
        let fog = entity_key(world.spawn(Fog::default()).id());
        let moved = entity_key(world.spawn(Transform::IDENTITY).id());

        with_world_scope(&mut world, || {
            let start = component(&fog, "Fog").unwrap().child("falloff").child("start");
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(write(&start, Field::Float(bad)).is_err());
            }
            assert!(matches!(read(&start), Ok(Read::Value(Field::Float(0.0)))), "a refused write changes nothing");

            let rotation = component(&moved, "Transform").unwrap().child("rotation");
            assert!(write(&rotation, Field::Math(Math::Quat(Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0)))).is_err());
            write(&rotation, Field::Math(Math::Quat(Quat::from_xyzw(0.0, 0.0, 0.0, 2.0)))).unwrap();
            let q = with_value(&rotation, |v| v.try_downcast_ref::<Quat>().copied()).unwrap().expect("a Quat");
            assert!((q.length() - 1.0).abs() < 1e-6, "rotations are normalized on the way in");
            Ok(())
        })
        .unwrap();
    }

    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct Door {
        state: DoorState,
        target: Option<f32>,
    }

    #[derive(Reflect, Default, PartialEq, Debug)]
    enum DoorState {
        #[default]
        Closed,
        Opening,
    }

    // CLAUDE
    /// an enum in a variant without fields reads as the variant's name, Option's None as null
    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn unit_variants_read_as_names() {
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        world.resource::<AppTypeRegistry>().write().register::<Door>();
        world.resource::<AppTypeRegistry>().write().register::<Fog>();
        let entity = world.spawn((Door::default(), Fog::default())).id();
        let door = entity_key(entity);

        with_world_scope(&mut world, || {
            let d = component(&door, "Door").unwrap();
            let name = |r: &LiveRef| match read(r) {
                Ok(Read::Value(field)) => field,
                _ => panic!("not a value"),
            };
            assert_eq!(name(&d.child("state")), Field::Str("Closed".into()));
            assert_eq!(name(&d.child("target")), Field::Null);
            write(&d.child("state"), Field::Str("Opening".into())).unwrap();
            assert_eq!(name(&d.child("state")), Field::Str("Opening".into()), "the cache was cleared by the variant change");
            assert!(matches!(read(&component(&door, "Fog").unwrap().child("falloff")), Ok(Read::Ref(_))), "a variant with fields is a ref");
            Ok(())
        })
        .unwrap();
        assert_eq!(world.get::<Door>(entity).unwrap().state, DoorState::Opening);
    }

    // CLAUDE
    /// a string-keyed map's values by key
    #[cfg_attr(not(target_family = "wasm"), test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    fn map_keys_are_fields() {
        #[derive(Component, Reflect, Default)]
        #[reflect(Component, Default)]
        struct Rows(HashMap<String, f32>);

        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        world.resource::<AppTypeRegistry>().write().register::<Rows>();
        let entity = world.spawn(Rows([("reveal".to_string(), 1.0)].into_iter().collect())).id();
        let rows = entity_key(entity);
        with_world_scope(&mut world, || {
            let r = component(&rows, "Rows").unwrap();
            write(&r.child("0").child("reveal"), Field::Float(2.0)).unwrap();
            assert!(matches!(read(&r.child("0").child("nope")), Ok(Read::Missing)));
            Ok(())
        })
        .unwrap();
        assert_eq!(world.get::<Rows>(entity).unwrap().0["reveal"], 2.0);
    }
}
