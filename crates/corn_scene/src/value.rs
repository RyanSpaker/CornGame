//! CLAUDE (claude-opus-5-5): values in scene files and how they patch reflected data.
//!
//! A [`Value`] is parsed without knowing its type; [`patch`] then walks the target's type info:
//! - structs take `{ a: 1; b.c: 2 }` (only listed fields change) or positional `[1, 2, 3]`
//! - tuple structs take `Name(a, b)` / `[a, b]`; a one-field one also takes its field's value directly
//! - enums take a variant: `Static`, `SameSceneName("a")`, `Cuboid { x_length: 2 }`; a one-field tuple
//!   variant also takes a struct body for its field (`Srgba { red: 1 }`). Switching variant starts its
//!   fields from Default
//! - `Option<T>` takes a bare `T` as `Some(T)`; `None` for none
//! - lists take `[a, b]`, which replaces the contents
//! - `default` anywhere: the type's whole Default
//! - `Handle<A>` (registered with corn_assets): `"path"` loads it, `name` is the file's `asset name: ..`,
//!   and data makes a new asset (registered inline): `{ base_color: #ff0000 }` patches `A::default()`
//! - `Entity`: `#name` or `select(selector, ..)`, which must match exactly one entity. a list of entities
//!   takes everything one matches. what they match is looked up before patching ([`EntityRefs`])
//!
//! Types the generic rules can't express get a parser hook, [`ReflectParseValue`] type data: implement
//! [`ParseValue`] and `#[reflect(ParseValue)]` on your own types, or [`RegisterValueParser`] for types
//! from other crates (which the orphan rule keeps from implementing the trait). A hook may decline
//! (`Ok(None)`) to let the generic rules handle a value.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::reflect::{
    enums::{DynamicEnum, DynamicVariant, VariantInfo},
    list::DynamicList,
    map::DynamicMap,
    structs::DynamicStruct,
    tuple::DynamicTuple,
    tuple_struct::DynamicTupleStruct,
    CreateTypeData, GetTypeRegistration, ReflectMut, ReflectRef, TypeInfo, TypeRegistry,
};
use corn_assets::{ReflectAssetPath, ReflectInlineAsset};

use crate::doc::Selector;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `2`, `-1.5`, `40px`, `50%`
    Num { value: f64, unit: Option<String> },
    Str(String),
    /// `#ffffff64`
    Hex(String),
    /// identifier or path: a unit variant, `true`, `None`, `default`
    Path(String),
    /// `Name(a, b)`; empty name for a bare `(a, b)`
    Call(String, Vec<Value>),
    /// `Name { a: 1; b.c: 2 }`; empty name for a bare `{ .. }`. keys are field paths
    Struct(String, Vec<(Vec<String>, Value)>),
    /// `[a, b]`
    List(Vec<Value>),
    /// space separated: `5px 10px`, for parser hooks
    Seq(Vec<Value>),
    /// `${ .. }`: index into the doc's inline code. replaced by its result before patching
    Script(usize),
    /// a script's result
    Json(serde_json::Value),
    /// `select(selector, ..)`: entities, for `Entity` fields (`#name` also is one there)
    Select(SelectorList),
}

// CLAUDE
/// the selectors of a `select(..)`; values compare by identity
#[derive(Debug, Clone)]
pub struct SelectorList(pub Arc<Vec<Selector>>);

impl PartialEq for SelectorList {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

// CLAUDE
/// what the entity references in a value (`#name`, `select(..)`) matched, found before patching
#[derive(Default)]
pub struct EntityRefs {
    pub names: HashMap<String, Vec<Entity>>,
    /// by the address of the `select(..)`'s selectors
    pub selects: HashMap<usize, Vec<Entity>>,
}

impl EntityRefs {
    /// the matches of an entity reference, None if `value` isn't one
    pub fn get(&self, value: &Value) -> Option<&[Entity]> {
        match value {
            Value::Hex(name) => self.names.get(name).map(Vec::as_slice),
            Value::Select(list) => self.selects.get(&(Arc::as_ptr(&list.0) as usize)).map(Vec::as_slice),
            _ => None,
        }
    }
}

impl Value {
    // CLAUDE
    /// whether it holds something that may be an entity reference: `select(..)`, or `#name` (which may
    /// also be a color)
    pub fn has_ref(&self) -> bool {
        match self {
            Value::Select(_) | Value::Hex(_) => true,
            Value::Call(_, items) | Value::List(items) | Value::Seq(items) => items.iter().any(Value::has_ref),
            Value::Struct(_, entries) => entries.iter().any(|(_, v)| v.has_ref()),
            _ => false,
        }
    }

    /// each possible entity reference in it, see [`Value::has_ref`]
    pub fn each_ref<'v>(&'v self, f: &mut impl FnMut(&'v Value)) {
        match self {
            Value::Select(_) | Value::Hex(_) => f(self),
            Value::Call(_, items) | Value::List(items) | Value::Seq(items) => items.iter().for_each(|v| v.each_ref(f)),
            Value::Struct(_, entries) => entries.iter().for_each(|(_, v)| v.each_ref(f)),
            _ => {}
        }
    }

    /// whether `${ .. }` is in it
    pub fn has_script(&self) -> bool {
        match self {
            Value::Script(_) => true,
            Value::Call(_, items) | Value::List(items) | Value::Seq(items) => items.iter().any(Value::has_script),
            Value::Struct(_, entries) => entries.iter().any(|(_, v)| v.has_script()),
            _ => false,
        }
    }

    /// a copy with each `${ .. }` replaced by `eval`'s result
    pub fn resolve_scripts(&self, eval: &mut impl FnMut(usize) -> Result<serde_json::Value, String>) -> Result<Value, String> {
        let mut list = |items: &[Value]| items.iter().map(|v| v.resolve_scripts(eval)).collect::<Result<Vec<_>, _>>();
        Ok(match self {
            Value::Script(index) => Value::Json(eval(*index)?),
            Value::Call(n, items) => Value::Call(n.clone(), list(items)?),
            Value::List(items) => Value::List(list(items)?),
            Value::Seq(items) => Value::Seq(list(items)?),
            Value::Struct(n, entries) => Value::Struct(
                n.clone(),
                entries.iter().map(|(k, v)| Ok((k.clone(), v.resolve_scripts(eval)?))).collect::<Result<_, String>>()?,
            ),
            other => other.clone(),
        })
    }

    fn describe(&self) -> String {
        match self {
            Value::Script(_) => "${..}".into(),
            Value::Json(json) => json.to_string(),
            Value::Num { value, unit } => format!("{value}{}", unit.as_deref().unwrap_or("")),
            Value::Str(s) => format!("{s:?}"),
            Value::Hex(h) => format!("#{h}"),
            Value::Path(p) => p.clone(),
            Value::Call(n, _) => format!("{n}(..)"),
            Value::Struct(n, _) => format!("{n} {{..}}"),
            Value::List(_) => "[..]".into(),
            Value::Seq(_) => "a list of values".into(),
            Value::Select(_) => "select(..)".into(),
        }
    }

    // CLAUDE
    /// a plain value as text, for attribute selectors: strings, names, numbers
    pub fn as_text(&self) -> Option<String> {
        match self {
            Value::Str(s) | Value::Path(s) => Some(s.clone()),
            Value::Num { value, unit } => Some(format!("{value}{}", unit.as_deref().unwrap_or(""))),
            _ => None,
        }
    }
}

/// what parsing a value may need besides the value
pub struct ParseCx<'a> {
    pub registry: &'a TypeRegistry,
    /// to load assets for handle fields; None where there's no asset server
    pub assets: Option<&'a AssetServer>,
    /// the file's `asset name: ..` definitions
    pub named: Option<&'a HashMap<String, UntypedHandle>>,
    /// patches with a script's result (JSON, as scripts write components); None without scripting
    pub json: Option<JsonPatch>,
    /// what the value's entity references matched; None where entities can't be referenced
    pub entities: Option<&'a EntityRefs>,
}

pub type JsonPatch = fn(&mut dyn PartialReflect, &serde_json::Value, &TypeRegistry, Option<&AssetServer>) -> Result<(), String>;

pub type ParseHook =
    Arc<dyn Fn(&Value, &ParseCx) -> Result<Option<Box<dyn PartialReflect>>, String> + Send + Sync>;

/// type data: parses this type's values ahead of the generic rules
#[derive(Clone)]
pub struct ReflectParseValue(pub ParseHook);

/// a type's own value parser, for `#[reflect(ParseValue)]`
pub trait ParseValue: Sized {
    /// None: not a value this parser handles, use the generic rules
    fn parse_value(value: &Value, cx: &ParseCx) -> Result<Option<Self>, String>;
}

impl<T: ParseValue + PartialReflect> CreateTypeData<T> for ReflectParseValue {
    fn create_type_data(_: ()) -> Self {
        ReflectParseValue(Arc::new(|value, cx| Ok(T::parse_value(value, cx)?.map(|t| Box::new(t) as _))))
    }
}

/// value parsers for types this crate can't implement [`ParseValue`] for
pub trait RegisterValueParser {
    fn register_value_parser<T: PartialReflect + GetTypeRegistration>(
        &mut self,
        parse: impl Fn(&Value, &ParseCx) -> Result<Option<T>, String> + Send + Sync + 'static,
    ) -> &mut Self;
}

impl RegisterValueParser for App {
    fn register_value_parser<T: PartialReflect + GetTypeRegistration>(
        &mut self,
        parse: impl Fn(&Value, &ParseCx) -> Result<Option<T>, String> + Send + Sync + 'static,
    ) -> &mut Self {
        let hook = ReflectParseValue(Arc::new(move |value, cx| Ok(parse(value, cx)?.map(|t| Box::new(t) as _))));
        let registry = self.world().resource::<AppTypeRegistry>().clone();
        let mut registry = registry.write();
        registry.register::<T>();
        registry.get_mut(TypeId::of::<T>()).unwrap().insert(hook);
        self
    }
}

/// patches `target` with `value` at the field path `path` (empty: the target itself)
pub fn patch_path(target: &mut dyn PartialReflect, path: &[String], value: &Value, cx: &ParseCx) -> Result<(), String> {
    let Some((field, rest)) = path.split_first() else { return patch(target, value, cx) };
    let type_name = target.reflect_type_path().to_string();
    let inner = match target.reflect_mut() {
        ReflectMut::Struct(s) => s.field_mut(field),
        ReflectMut::TupleStruct(s) => field.parse().ok().and_then(|i| s.field_mut(i)),
        ReflectMut::Tuple(t) => field.parse().ok().and_then(|i| t.field_mut(i)),
        ReflectMut::Enum(e) => match field.parse() {
            Ok(i) => e.field_at_mut(i),
            Err(_) => e.field_mut(field),
        },
        _ => None,
    };
    let inner = inner.ok_or_else(|| format!("`{type_name}` has no field `{field}`"))?;
    patch_path(inner, rest, value, cx).map_err(|e| format!("{field}: {e}"))
}

/// patches `target` with `value`, see the module docs for the rules
pub fn patch(target: &mut dyn PartialReflect, value: &Value, cx: &ParseCx) -> Result<(), String> {
    let info = target
        .get_represented_type_info()
        .ok_or_else(|| format!("`{}` has no type info", target.reflect_type_path()))?;
    let type_id = info.type_id();
    let apply = |target: &mut dyn PartialReflect, v: Box<dyn PartialReflect>| {
        target.try_apply(v.as_ref()).map_err(|e| e.to_string())
    };

    match value {
        Value::Json(json) => {
            let json_patch = cx.json.ok_or("scripts need corn_scene's `scripting` feature")?;
            return json_patch(target, json, cx.registry, cx.assets);
        }
        Value::Script(_) => return Err("`${..}` was not run".into()),
        _ => {}
    }
    if type_id == TypeId::of::<Entity>() {
        return match entity_ref(value, cx)? {
            [one] => apply(target, Box::new(*one)),
            [] => Err(format!("{} matches no entity", value.describe())),
            many => Err(format!("{} matches {} entities, it must match one", value.describe(), many.len())),
        };
    }
    if let Some(hook) = cx.registry.get_type_data::<ReflectParseValue>(type_id) {
        if let Some(v) = (hook.0)(value, cx)? {
            return apply(target, v);
        }
    }
    if matches!(value, Value::Path(p) if p == "default") {
        return apply(target, default_of(info, cx.registry)?);
    }
    if let Some(paths) = cx.registry.get_type_data::<ReflectAssetPath>(type_id) {
        return apply(target, handle(paths, value, cx)?);
    }
    if let TypeInfo::Enum(_) = info {
        return patch_enum(target, info, value, cx);
    }
    let expected = || format!("`{}` can't be {}", info.type_path_table().short_path(), value.describe());
    let short = info.type_path_table().short_path();
    let named = |name: &str| name.is_empty() || name == short || name == info.type_path_table().ident().unwrap_or("");

    let fields = field_count(target);
    match (target.reflect_ref(), value) {
        (ReflectRef::Struct(_), Value::Struct(name, entries)) if named(name) => {
            for (path, v) in entries {
                patch_path(target, path, v, cx)?;
            }
            Ok(())
        }
        (ReflectRef::Struct(_), Value::List(items)) => positional(target, items, cx),
        (ReflectRef::TupleStruct(_), Value::Call(name, items)) if named(name) => positional(target, items, cx),
        (ReflectRef::TupleStruct(_), Value::List(items)) if fields != 1 => positional(target, items, cx),
        (ReflectRef::TupleStruct(_), value) if fields == 1 => patch(field_mut(target, 0).unwrap(), value, cx),
        (ReflectRef::Tuple(_), Value::Call(name, items)) if name.is_empty() => positional(target, items, cx),
        (ReflectRef::Tuple(_) | ReflectRef::Array(_), Value::List(items)) => positional(target, items, cx),
        (ReflectRef::List(_), Value::List(items)) => {
            let TypeInfo::List(list_info) = info else { return Err(expected()) };
            let item_info = cx
                .registry
                .get_type_info(list_info.item_ty().id())
                .ok_or_else(|| format!("`{}` is not registered", list_info.item_ty().path()))?;
            let ReflectMut::List(l) = target.reflect_mut() else { unreachable!() };
            l.drain();
            for (i, item) in items.iter().enumerate() {
                let mut new = default_of(item_info, cx.registry)?;
                patch(new.as_mut(), item, cx).map_err(|e| format!("[{i}]: {e}"))?;
                l.push(new);
            }
            Ok(())
        }
        // a list of entities: everything the reference matches
        (ReflectRef::List(_), Value::Select(_) | Value::Hex(_))
            if matches!(info, TypeInfo::List(l) if l.item_ty().id() == TypeId::of::<Entity>()) =>
        {
            let entities = entity_ref(value, cx)?.to_vec();
            let ReflectMut::List(l) = target.reflect_mut() else { unreachable!() };
            l.drain();
            entities.into_iter().for_each(|e| l.push(Box::new(e)));
            Ok(())
        }
        (ReflectRef::Opaque(_), value) => match primitive(type_id, value) {
            Some(v) => apply(target, v?),
            None => Err(format!("no parser for `{}`: register one (RegisterValueParser)", info.type_path())),
        },
        _ => Err(expected()),
    }
}

// CLAUDE
/// the entities `#name` or `select(..)` matched
fn entity_ref<'c>(value: &Value, cx: &'c ParseCx) -> Result<&'c [Entity], String> {
    if !matches!(value, Value::Hex(_) | Value::Select(_)) {
        return Err(format!("an entity is `#name` or `select(selector)`, got {}", value.describe()));
    }
    let refs = cx.entities.ok_or("entities can't be referenced here")?;
    refs.get(value).ok_or_else(|| format!("{} was not looked up", value.describe()))
}

fn field_count(target: &dyn PartialReflect) -> usize {
    match target.reflect_ref() {
        ReflectRef::Struct(s) => s.field_len(),
        ReflectRef::TupleStruct(s) => s.field_len(),
        ReflectRef::Tuple(t) => t.field_len(),
        ReflectRef::Array(a) => a.len(),
        ReflectRef::Enum(e) => e.field_len(),
        _ => 0,
    }
}

fn field_mut(target: &mut dyn PartialReflect, i: usize) -> Option<&mut dyn PartialReflect> {
    match target.reflect_mut() {
        ReflectMut::Struct(s) => s.field_at_mut(i),
        ReflectMut::TupleStruct(s) => s.field_mut(i),
        ReflectMut::Tuple(t) => t.field_mut(i),
        ReflectMut::Array(a) => a.get_mut(i),
        ReflectMut::Enum(e) => e.field_at_mut(i),
        _ => None,
    }
}

/// patches fields in order
fn positional(target: &mut dyn PartialReflect, items: &[Value], cx: &ParseCx) -> Result<(), String> {
    let len = field_count(target);
    if items.len() > len {
        return Err(format!("{} values for {len} fields", items.len()));
    }
    for (i, item) in items.iter().enumerate() {
        patch(field_mut(target, i).unwrap(), item, cx).map_err(|e| format!("{i}: {e}"))?;
    }
    Ok(())
}

fn patch_enum(target: &mut dyn PartialReflect, info: &'static TypeInfo, value: &Value, cx: &ParseCx) -> Result<(), String> {
    let TypeInfo::Enum(enum_info) = info else { unreachable!() };
    let is_option = info.type_path().starts_with("core::option::Option<");
    let last = |p: &str| p.rsplit("::").next().unwrap().to_string();
    let some = || ("Some".to_string(), Some(Value::Call(String::new(), vec![value.clone()])));
    let (variant, payload) = match value {
        Value::Path(p) => (last(p), None),
        Value::Call(p, _) | Value::Struct(p, _) if !p.is_empty() => (last(p), Some(value.clone())),
        _ if is_option => some(),
        _ => return Err(format!("`{}` needs a variant, got {}", enum_info.type_path_table().short_path(), value.describe())),
    };
    // a bare T for Option<T>, unless it names Some/None
    let (variant, payload) = if is_option && variant != "Some" && variant != "None" { some() } else { (variant, payload) };

    let Some(variant_info) = enum_info.variant(&variant) else {
        let names: Vec<&str> = enum_info.iter().map(VariantInfo::name).collect();
        return Err(format!("`{}` has no variant `{variant}` (has {})", enum_info.type_path_table().short_path(), names.join(", ")));
    };
    let ReflectRef::Enum(current) = target.reflect_ref() else { unreachable!() };
    if current.variant_name() != variant {
        let mut fresh = DynamicEnum::new(variant.clone(), default_variant(variant_info, cx.registry)?);
        fresh.set_represented_type(Some(info));
        target.try_apply(&fresh).map_err(|e| e.to_string())?;
    }
    match (variant_info, payload) {
        (VariantInfo::Unit(_), None) => Ok(()),
        (VariantInfo::Tuple(_), Some(Value::Call(_, items))) => positional(target, &items, cx),
        (VariantInfo::Tuple(t), Some(Value::Struct(_, entries))) if t.field_len() == 1 => {
            patch(field_mut(target, 0).unwrap(), &Value::Struct(String::new(), entries), cx)
        }
        (VariantInfo::Struct(_), Some(Value::Struct(_, entries))) => {
            for (path, v) in &entries {
                patch_path(target, path, v, cx)?;
            }
            Ok(())
        }
        // a struct or tuple variant may be named alone, keeping Default fields
        (VariantInfo::Tuple(_) | VariantInfo::Struct(_), None) => Ok(()),
        (_, _) => Err(format!("wrong form for variant `{variant}`: {}", value.describe())),
    }
}

fn default_variant(variant: &VariantInfo, registry: &TypeRegistry) -> Result<DynamicVariant, String> {
    let field_default = |id: TypeId, path: &str| {
        let info = registry.get_type_info(id).ok_or_else(|| format!("`{path}` is not registered"))?;
        default_of(info, registry)
    };
    Ok(match variant {
        VariantInfo::Unit(_) => DynamicVariant::Unit,
        VariantInfo::Tuple(t) => {
            let mut tuple = DynamicTuple::default();
            for f in t.iter() {
                tuple.insert_boxed(field_default(f.type_id(), f.type_path())?);
            }
            DynamicVariant::Tuple(tuple)
        }
        VariantInfo::Struct(s) => {
            let mut st = DynamicStruct::default();
            for f in s.iter() {
                st.insert_boxed(f.name(), field_default(f.type_id(), f.type_path())?);
            }
            DynamicVariant::Struct(st)
        }
    })
}

/// a type's Default: its `ReflectDefault`, or built from its fields' defaults; Option is None,
/// collections are empty
pub fn default_of(info: &'static TypeInfo, registry: &TypeRegistry) -> Result<Box<dyn PartialReflect>, String> {
    if let Some(d) = registry.get_type_data::<ReflectDefault>(info.type_id()) {
        return Ok(d.default().into_partial_reflect());
    }
    // until a reference sets it
    if info.type_id() == TypeId::of::<Entity>() {
        return Ok(Box::new(Entity::PLACEHOLDER));
    }
    let field = |id: TypeId, path: &str| {
        let info = registry.get_type_info(id).ok_or_else(|| format!("`{path}` is not registered"))?;
        default_of(info, registry)
    };
    let typed = Some(info);
    Ok(match info {
        TypeInfo::Enum(_) if info.type_path().starts_with("core::option::Option<") => {
            let mut none = DynamicEnum::new("None", DynamicVariant::Unit);
            none.set_represented_type(typed);
            Box::new(none)
        }
        TypeInfo::Struct(s) => {
            let mut st = DynamicStruct::default();
            for f in s.iter() {
                st.insert_boxed(f.name(), field(f.type_id(), f.type_path())?);
            }
            st.set_represented_type(typed);
            Box::new(st)
        }
        TypeInfo::TupleStruct(s) => {
            let mut st = DynamicTupleStruct::default();
            for f in s.iter() {
                st.insert_boxed(field(f.type_id(), f.type_path())?);
            }
            st.set_represented_type(typed);
            Box::new(st)
        }
        TypeInfo::Tuple(t) => {
            let mut tuple = DynamicTuple::default();
            for f in t.iter() {
                tuple.insert_boxed(field(f.type_id(), f.type_path())?);
            }
            tuple.set_represented_type(typed);
            Box::new(tuple)
        }
        TypeInfo::List(_) => {
            let mut list = DynamicList::default();
            list.set_represented_type(typed);
            Box::new(list)
        }
        TypeInfo::Map(_) => {
            let mut map = DynamicMap::default();
            map.set_represented_type(typed);
            Box::new(map)
        }
        _ => return Err(format!("`{}` has no Default", info.type_path())),
    })
}

// CLAUDE
/// the type at a field path, None if the path isn't there (an enum's field may be in any variant)
pub(crate) fn type_at(registry: &TypeRegistry, mut info: &'static TypeInfo, path: &[String]) -> Option<&'static TypeInfo> {
    for field in path {
        let id = match info {
            TypeInfo::Struct(s) => s.field(field)?.type_id(),
            TypeInfo::TupleStruct(s) => s.field_at(field.parse().ok()?)?.type_id(),
            TypeInfo::Tuple(t) => t.field_at(field.parse().ok()?)?.type_id(),
            TypeInfo::Enum(e) => e.iter().find_map(|v| match v {
                VariantInfo::Struct(s) => s.field(field).map(|f| f.type_id()),
                VariantInfo::Tuple(t) => t.field_at(field.parse().ok()?).map(|f| f.type_id()),
                VariantInfo::Unit(_) => None,
            })?,
            _ => return None,
        };
        info = registry.get_type_info(id)?;
    }
    Some(info)
}

// CLAUDE
/// whether a value of this type can hold an `Entity`
pub(crate) fn holds_entity(registry: &TypeRegistry, info: &'static TypeInfo) -> bool {
    fn walk(registry: &TypeRegistry, id: TypeId, seen: &mut Vec<TypeId>) -> bool {
        if id == TypeId::of::<Entity>() {
            return true;
        }
        if seen.contains(&id) {
            return false;
        }
        seen.push(id);
        let Some(info) = registry.get_type_info(id) else { return false };
        let mut ids: Vec<TypeId> = vec![];
        match info {
            TypeInfo::Struct(s) => ids.extend(s.iter().map(|f| f.type_id())),
            TypeInfo::TupleStruct(s) => ids.extend(s.iter().map(|f| f.type_id())),
            TypeInfo::Tuple(t) => ids.extend(t.iter().map(|f| f.type_id())),
            TypeInfo::List(l) => ids.push(l.item_ty().id()),
            TypeInfo::Array(a) => ids.push(a.item_ty().id()),
            TypeInfo::Set(s) => ids.push(s.value_ty().id()),
            TypeInfo::Map(m) => ids.extend([m.key_ty().id(), m.value_ty().id()]),
            TypeInfo::Enum(e) => {
                for v in e.iter() {
                    match v {
                        VariantInfo::Struct(s) => ids.extend(s.iter().map(|f| f.type_id())),
                        VariantInfo::Tuple(t) => ids.extend(t.iter().map(|f| f.type_id())),
                        VariantInfo::Unit(_) => {}
                    }
                }
            }
            _ => {}
        }
        ids.into_iter().any(|id| walk(registry, id, seen))
    }
    walk(registry, info.type_id(), &mut vec![])
}

fn primitive(type_id: TypeId, value: &Value) -> Option<Result<Box<dyn PartialReflect>, String>> {
    macro_rules! numbers {
        ($($t:ty),*) => {$(
            if type_id == TypeId::of::<$t>() {
                return Some(match value {
                    Value::Num { value, unit: None } => Ok(Box::new(*value as $t)),
                    Value::Num { unit: Some(unit), .. } => Err(format!("unexpected unit `{unit}`")),
                    v => Err(format!("expected a number, got {}", v.describe())),
                });
            }
        )*};
    }
    numbers!(f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);
    if type_id == TypeId::of::<bool>() {
        return Some(match value {
            Value::Path(p) if p == "true" => Ok(Box::new(true)),
            Value::Path(p) if p == "false" => Ok(Box::new(false)),
            v => Err(format!("expected true or false, got {}", v.describe())),
        });
    }
    if type_id == TypeId::of::<String>() {
        return Some(match value {
            Value::Str(s) => Ok(Box::new(s.clone())),
            v => Err(format!("expected a \"string\", got {}", v.describe())),
        });
    }
    if type_id == TypeId::of::<char>() {
        let mut chars = match value {
            Value::Str(s) => s.chars(),
            _ => "".chars(),
        };
        return Some(match (chars.next(), chars.next()) {
            (Some(c), None) => Ok(Box::new(c)),
            _ => Err(format!("expected one character, got {}", value.describe())),
        });
    }
    None
}

// CLAUDE
/// the parsers every app gets: `Name` from a string or identifier, `Color` from `#hex`
pub(crate) fn register_builtin(app: &mut App) {
    app.register_value_parser::<Name>(|value, _| match value {
        Value::Str(s) | Value::Path(s) => Ok(Some(Name::new(s.clone()))),
        _ => Ok(None),
    });
    app.register_value_parser::<Color>(|value, _| match value {
        Value::Hex(h) => Srgba::hex(h).map(|c| Some(Color::Srgba(c))).map_err(|e| format!("#{h}: {e}")),
        _ => Ok(None),
    });
}

// CLAUDE
/// a `Handle<A>` (registered with corn_assets) from `"path"`, the name of the file's `asset`, or data
/// for a new `A` (registered inline)
fn handle(paths: &ReflectAssetPath, value: &Value, cx: &ParseCx) -> Result<Box<dyn PartialReflect>, String> {
    let asset_name = || cx.registry.get_type_info(paths.asset).map_or("?", |i| i.type_path_table().short_path());
    let need_assets = || cx.assets.ok_or("no asset server to load assets with");
    match value {
        Value::Str(path) => Ok(paths.load(need_assets()?, path.clone())),
        Value::Path(name) => {
            let found = cx.named.and_then(|named| named.get(name)).ok_or_else(|| {
                format!("no `asset {name}` in this file (a path to an asset is a \"string\")")
            })?;
            paths.typed(found.clone()).ok_or_else(|| format!("`{name}` is not a `{}`", asset_name()))
        }
        data => {
            let inline = cx.registry.get_type_data::<ReflectInlineAsset>(paths.asset).ok_or_else(|| {
                format!("a `{}` can't be made from data here (register it with corn_assets::register_inline or register_recipe)", asset_name())
            })?;
            let info = cx.registry.get_type_info(inline.data).ok_or("the asset's data type is not registered")?;
            let mut asset = default_of(info, cx.registry)?;
            patch(asset.as_mut(), data, cx)?;
            inline.add(need_assets()?, asset.as_ref())
        }
    }
}

// CLAUDE
/// `Cuboid(1, 2, 1)` for a mesh: a recipe of that one shape (see corn_assets::mesh)
#[cfg(feature = "mesh")]
pub(crate) fn register_mesh(app: &mut App) {
    use corn_assets::mesh::MeshRecipe;
    app.register_value_parser::<MeshRecipe>(|value, _| {
        let Value::Call(name, args) = value else { return Ok(None) };
        if name.is_empty() {
            return Ok(None);
        }
        let args = args
            .iter()
            .map(|a| match a {
                Value::Num { value, unit: None } => Ok(*value as f32),
                other => Err(format!("`{name}` takes numbers, got {}", other.describe())),
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(MeshRecipe::shape(name.clone(), args)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Reflect, Default, Debug, PartialEq)]
    struct Inner {
        a: f32,
        b: f32,
    }

    #[derive(Reflect, Default, Debug, PartialEq)]
    enum Shape {
        #[default]
        Empty,
        Cuboid { x: f32, y: f32 },
        Named(String),
        Wrapped(Inner),
    }

    #[derive(Reflect, Default, Debug, PartialEq)]
    struct Big {
        n: u32,
        flag: bool,
        text: String,
        inner: Inner,
        shape: Shape,
        maybe: Option<f32>,
        list: Vec<Inner>,
        pair: (u8, u8),
        newtype: Wrap,
        color: Color,
    }

    #[derive(Reflect, Default, Debug, PartialEq)]
    struct Wrap(f32);

    fn registry() -> TypeRegistry {
        let mut app = App::new();
        app.register_type::<Big>();
        register_builtin(&mut app);
        let registry = app.world().resource::<AppTypeRegistry>().read();
        let mut out = TypeRegistry::new();
        for r in registry.iter() {
            out.add_registration(r.clone());
        }
        out
    }

    fn num(v: f64) -> Value {
        Value::Num { value: v, unit: None }
    }
    fn path(p: &str) -> Value {
        Value::Path(p.into())
    }
    fn entry(k: &str, v: Value) -> (Vec<String>, Value) {
        (k.split('.').map(str::to_string).collect(), v)
    }

    #[test]
    fn patches() {
        let registry = registry();
        let cx = ParseCx { registry: &registry, assets: None, named: None, json: None, entities: None };
        let mut big = Big::default();
        let value = Value::Struct(String::new(), vec![
            entry("n", num(3.0)),
            entry("flag", path("true")),
            entry("text", Value::Str("hi".into())),
            entry("inner.b", num(2.0)),
            entry("shape", Value::Struct("Cuboid".into(), vec![entry("x", num(1.0))])),
            entry("maybe", num(5.0)),
            entry("list", Value::List(vec![Value::Struct(String::new(), vec![entry("a", num(1.0))]), Value::List(vec![num(7.0), num(8.0)])])),
            entry("pair", Value::List(vec![num(1.0), num(2.0)])),
            entry("newtype", num(4.0)),
            entry("color", Value::Hex("ff0000".into())),
        ]);
        patch(&mut big, &value, &cx).unwrap();
        assert_eq!(big, Big {
            n: 3,
            flag: true,
            text: "hi".into(),
            inner: Inner { a: 0.0, b: 2.0 },
            shape: Shape::Cuboid { x: 1.0, y: 0.0 },
            maybe: Some(5.0),
            list: vec![Inner { a: 1.0, b: 0.0 }, Inner { a: 7.0, b: 8.0 }],
            pair: (1, 2),
            newtype: Wrap(4.0),
            color: Color::srgb(1.0, 0.0, 0.0),
        });

        patch_path(&mut big, &["shape".into()], &Value::Call("Named".into(), vec![Value::Str("x".into())]), &cx).unwrap();
        assert_eq!(big.shape, Shape::Named("x".into()));
        patch_path(&mut big, &["shape".into()], &Value::Struct("Wrapped".into(), vec![entry("a", num(1.0))]), &cx).unwrap();
        assert_eq!(big.shape, Shape::Wrapped(Inner { a: 1.0, b: 0.0 }));
        patch_path(&mut big, &["maybe".into()], &path("None"), &cx).unwrap();
        assert_eq!(big.maybe, None);
        patch_path(&mut big, &["inner".into()], &path("default"), &cx).unwrap();
        assert_eq!(big.inner, Inner::default());

        let err = patch_path(&mut big, &["shape".into()], &path("Round"), &cx).unwrap_err();
        assert!(err.contains("no variant `Round`"), "{err}");
        let err = patch_path(&mut big, &["n".into()], &Value::Num { value: 1.0, unit: Some("px".into()) }, &cx).unwrap_err();
        assert!(err.contains("unit"), "{err}");
    }

    #[test]
    fn defaults_without_reflect_default() {
        #[derive(Reflect, Debug, PartialEq)]
        struct NoDefault {
            a: f32,
            o: Option<u8>,
        }
        let mut registry = registry();
        registry.register::<NoDefault>();
        let info = registry.get_type_info(TypeId::of::<NoDefault>()).unwrap();
        let value = default_of(info, &registry).unwrap();
        assert_eq!(NoDefault::from_reflect(value.as_ref()), Some(NoDefault { a: 0.0, o: None }));
    }
}
