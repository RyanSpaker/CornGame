//! CLAUDE (claude-opus-5-5): how reflected values cross to scripts, beyond plain serde:
//! - `Entity` is its id string (what handlers receive), both ways, also in newtypes like `ChildOf`
//! - a `Handle<A>` is its asset path: writing a path loads it, reading gives the path it was loaded from.
//!   an asset without a path (made in code, or inline) reads as an id, `asset:...`, which `asset()` takes
//!   and writing back keeps, but which can't be assigned elsewhere. handle types must be registered
//!   with [`register`] (from corn_assets, which scene files share; the app registers its own).
//! - writing an object instead of a path makes a new asset, for types registered with
//!   [`register_inline`] (the object patches `A::default()`) or [`register_with`] (a function makes it,
//!   scripts only):
//!   `MeshMaterial3d<StandardMaterial>: { unlit: true }`

use std::any::TypeId;

use bevy::{
    prelude::*,
    reflect::{
        serde::{ReflectDeserializerProcessor, ReflectSerializerProcessor},
        tuple_struct::DynamicTupleStruct, ReflectRef, TypeInfo, TypeRegistration, TypeRegistry,
    },
};
use corn_assets::{handle_key, ReflectAssetPath, ReflectInlineAsset};
use serde::{de::Error, Deserialize, Deserializer, Serializer};

pub use corn_assets::{is_handle, parse_asset_id, register, register_inline};

/// makes an asset from script data, giving its handle
pub type CreateAsset = fn(&AssetServer, &serde_json::Value, &TypeRegistry) -> Result<Box<dyn PartialReflect>, String>;

// CLAUDE
/// also let an object make a new asset with `create`. scripts only: scene files have parser hooks
pub fn register_with<A: Asset>(app: &mut App, create: CreateAsset) {
    register::<A>(app);
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    registry.write().get_mut(TypeId::of::<Handle<A>>()).unwrap().insert(ScriptCreateAsset(create));
}

/// type data on `Handle<A>`: makes the asset from script data
#[derive(Clone)]
pub struct ScriptCreateAsset(CreateAsset);

// CLAUDE
/// an object written to a handle: its `create`, or `A::default()` patched with the object
fn create_asset(
    paths: &ReflectAssetPath,
    registration: &TypeRegistration,
    registry: &TypeRegistry,
    assets: &AssetServer,
    json: &serde_json::Value,
) -> Result<Box<dyn PartialReflect>, String> {
    if let Some(ScriptCreateAsset(create)) = registration.data::<ScriptCreateAsset>() {
        return create(assets, json, registry);
    }
    let Some(inline) = registry.get_type_data::<ReflectInlineAsset>(paths.asset) else {
        return Err(format!("expected an asset path, got {json}"));
    };
    let data = registry.get(inline.data).ok_or("the asset's data type is not registered")?;
    let value = super::value_from_json(data, registry, Some(assets), json)?;
    inline.add(assets, value.as_ref())
}

// CLAUDE
/// a serde processor for the reflect (de)serializers. without an asset server, paths can't be loaded
pub struct ScriptValues<'a>(pub Option<&'a AssetServer>);

impl ReflectDeserializerProcessor for ScriptValues<'_> {
    fn try_deserialize<'de, D: Deserializer<'de>>(
        &mut self,
        registration: &TypeRegistration,
        registry: &TypeRegistry,
        deserializer: D,
    ) -> Result<Result<Box<dyn PartialReflect>, D>, D::Error> {
        if registration.type_id() == TypeId::of::<Entity>() {
            let id = String::deserialize(deserializer)?;
            return super::parse_entity(&id).map(|e| Ok(Box::new(e) as _)).map_err(D::Error::custom);
        }
        if entity_newtype(registration.type_info()) {
            let entity = super::parse_entity(&String::deserialize(deserializer)?).map_err(D::Error::custom)?;
            let mut value = DynamicTupleStruct::default();
            value.insert(entity);
            value.set_represented_type(Some(registration.type_info()));
            return Ok(Ok(Box::new(value)));
        }
        // CLAUDE: a script's mesh where a mesh recipe goes (`asset post: Mesh ${ mesh()... }`)
        #[cfg(feature = "mesh")]
        if registration.type_id() == TypeId::of::<corn_assets::mesh::MeshRecipe>() {
            use bevy::reflect::serde::TypedReflectDeserializer;
            use serde::de::DeserializeSeed;
            let json = serde_json::Value::deserialize(deserializer)?;
            if let Some(baked) = json.as_str().and_then(super::mesh::baked) {
                let ops = vec![corn_assets::mesh::MeshOp::Baked(corn_assets::mesh::bake(baked.map_err(D::Error::custom)?))];
                return Ok(Ok(Box::new(corn_assets::mesh::MeshRecipe { ops })));
            }
            return TypedReflectDeserializer::new(registration, registry).deserialize(json).map(Ok).map_err(D::Error::custom);
        }
        let (Some(assets), Some(paths)) = (self.0, registration.data::<ReflectAssetPath>()) else {
            return Ok(Err(deserializer));
        };
        match serde_json::Value::deserialize(deserializer)? {
            // CLAUDE: a uuid handle (made in code at a fixed id) can be named anywhere; other ids can't make
            // a handle, so they stay where they are
            serde_json::Value::String(id) if id.starts_with("asset:") => match parse_asset_id(&id, paths.asset) {
                Some(bevy::asset::UntypedAssetId::Uuid { type_id, uuid }) => {
                    Ok(Ok(paths.typed(bevy::asset::UntypedHandle::Uuid { type_id, uuid }).expect("the handle's own type")))
                }
                _ => Err(D::Error::custom(format!("`{id}` has no path: it can only stay where it is"))),
            },
            #[cfg(feature = "mesh")]
            serde_json::Value::String(key) if key.starts_with("mesh:") && paths.asset == TypeId::of::<Mesh>() => {
                let handle = super::mesh::handle(&key, assets).unwrap_or_else(|| Err(format!("`{key}` is not a mesh")));
                handle.map(|h| paths.typed(h.untyped()).expect("a mesh handle")).map(Ok).map_err(D::Error::custom)
            }
            serde_json::Value::String(path) => Ok(Ok(paths.load(assets, path))),
            json => create_asset(paths, registration, registry, assets, &json).map(Ok).map_err(D::Error::custom),
        }
    }
}

impl ReflectSerializerProcessor for ScriptValues<'_> {
    fn try_serialize<S: Serializer>(
        &self,
        value: &dyn PartialReflect,
        registry: &TypeRegistry,
        serializer: S,
    ) -> Result<Result<S::Ok, S>, S::Error> {
        if let Some(entity) = value.try_downcast_ref::<Entity>() {
            return serializer.serialize_str(&super::entity_key(*entity)).map(Ok);
        }
        if value.get_represented_type_info().is_some_and(entity_newtype) {
            if let ReflectRef::TupleStruct(newtype) = value.reflect_ref() {
                if let Some(entity) = newtype.field(0).and_then(|e| e.try_downcast_ref::<Entity>()) {
                    return serializer.serialize_str(&super::entity_key(*entity)).map(Ok);
                }
            }
        }
        match handle_key(value, registry) {
            Some(key) => serializer.serialize_str(&key).map(Ok),
            None => Ok(Err(serializer)),
        }
    }
}

/// is `json` this handle as it reads, or null: writing it changes nothing
pub fn is_unchanged_handle(value: &dyn PartialReflect, json: &serde_json::Value, registry: &TypeRegistry) -> bool {
    match (handle_key(value, registry), json) {
        (Some(_), serde_json::Value::Null) => true,
        (Some(key), serde_json::Value::String(s)) => key == *s,
        _ => false,
    }
}

/// a tuple struct around one entity, like the relationship `ChildOf`: the entity's id string, both ways.
/// its own serde, if it has one, would give the entity's bits as a number
fn entity_newtype(info: &TypeInfo) -> bool {
    matches!(info, TypeInfo::TupleStruct(s) if s.field_len() == 1 && s.field_at(0).is_some_and(|f| f.type_id() == TypeId::of::<Entity>()))
}
