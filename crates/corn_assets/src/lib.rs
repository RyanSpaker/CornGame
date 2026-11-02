//! CLAUDE (claude-opus-5-5): which `Handle<A>` fields data formats (scripts, scene files) may write, and how.
//! Registering a type here covers every format:
//! - [`register`]: a handle is written as an asset path, which loads it, and reads as that path.
//!   an asset without a path (made in code, or inline) reads as an id, `asset:...` ([`parse_asset_id`])
//! - [`register_inline`]: data instead of a path makes a new asset, `A::default()` patched with the data
//! - [`register_recipe`]: data instead of a path is a recipe `R` (`R::default()` patched with the data),
//!   which a function turns into the asset: a mesh from shapes ([`mesh`])
//!
//! Each format patches with its own data; [`ReflectInlineAsset`] (on `A` itself) names the type the data
//! patches and turns the patched value into an asset.

use std::any::TypeId;
use std::sync::Arc;

use bevy::{
    asset::{uuid::Uuid, AssetIndex, LoadContext, UntypedAssetId},
    prelude::*,
    reflect::{CreateTypeData, GetTypeRegistration, TypeRegistry},
};

#[cfg(feature = "mesh")]
pub mod mesh;

/// let `Handle<A>` be written as an asset path
pub fn register<A: Asset>(app: &mut App) {
    app.register_type::<Handle<A>>()
        .register_type_data::<Handle<A>, ReflectAssetPath>();
}

/// also let data make a new `A`: `A::default()` patched with it
pub fn register_inline<A: Asset + FromReflect + Default + GetTypeRegistration>(app: &mut App) {
    register_recipe::<A, A>(app, Ok);
}

// CLAUDE
/// also let data make a new `A`: `R::default()` patched with it, then built with `build`
pub fn register_recipe<A, R>(app: &mut App, build: fn(R) -> Result<A, String>)
where
    A: Asset + GetTypeRegistration,
    R: FromReflect + TypePath + Default + GetTypeRegistration,
{
    register::<A>(app);
    app.register_type::<A>().register_type::<R>();
    let make = move |value: &dyn PartialReflect| -> Result<A, String> {
        let recipe = R::from_reflect(value).ok_or_else(|| format!("the data doesn't make a `{}`", R::type_path()))?;
        build(recipe)
    };
    let data = ReflectInlineAsset {
        data: TypeId::of::<R>(),
        add: Arc::new(move |assets, value| Ok(Box::new(assets.add(make(value)?)))),
        add_labeled: Arc::new(move |load_context, label, value| Ok(load_context.add_labeled_asset(label, make(value)?).untyped())),
        label_handle: |load_context, label| load_context.get_label_handle::<A>(label).untyped(),
    };
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    registry.write().get_mut(TypeId::of::<A>()).unwrap().insert(data);
}

// CLAUDE
/// type data on `Handle<A>`
#[derive(Clone)]
pub struct ReflectAssetPath {
    /// `A`
    pub asset: TypeId,
    load: fn(&AssetServer, String) -> Box<dyn PartialReflect>,
    key: fn(&dyn PartialReflect) -> Option<String>,
    typed: fn(UntypedHandle) -> Box<dyn PartialReflect>,
}

impl ReflectAssetPath {
    /// the handle that loads `path`
    pub fn load(&self, assets: &AssetServer, path: String) -> Box<dyn PartialReflect> {
        (self.load)(assets, path)
    }

    /// what the handle reads as: its path, or `asset:...` without one
    pub fn key(&self, handle: &dyn PartialReflect) -> Option<String> {
        (self.key)(handle)
    }

    /// `handle` as a `Handle<A>`, if it is one to an `A`
    pub fn typed(&self, handle: UntypedHandle) -> Option<Box<dyn PartialReflect>> {
        (handle.type_id() == self.asset).then(|| (self.typed)(handle))
    }
}

impl<A: Asset> CreateTypeData<Handle<A>> for ReflectAssetPath {
    fn create_type_data(_: ()) -> Self {
        Self {
            asset: TypeId::of::<A>(),
            load: |assets, path| Box::new(assets.load::<A>(path)),
            key: |value| {
                let handle = value.try_downcast_ref::<Handle<A>>()?;
                Some(handle.path().map_or_else(|| asset_id_key(handle.id().untyped()), |p| p.to_string()))
            },
            typed: |handle| Box::new(handle.typed::<A>()),
        }
    }
}

type AddFn = Arc<dyn Fn(&AssetServer, &dyn PartialReflect) -> Result<Box<dyn PartialReflect>, String> + Send + Sync>;
type AddLabeledFn = Arc<dyn Fn(&mut LoadContext, String, &dyn PartialReflect) -> Result<UntypedHandle, String> + Send + Sync>;

// CLAUDE
/// type data on an asset type `A`: makes an `A` from a reflected value of [`Self::data`] (usually that
/// type's default, patched)
#[derive(Clone)]
pub struct ReflectInlineAsset {
    /// the type data for a new `A` is: `A` itself, or its recipe
    pub data: TypeId,
    add: AddFn,
    add_labeled: AddLabeledFn,
    label_handle: fn(&mut LoadContext, String) -> UntypedHandle,
}

impl ReflectInlineAsset {
    /// a new asset, giving its `Handle<A>`
    pub fn add(&self, assets: &AssetServer, value: &dyn PartialReflect) -> Result<Box<dyn PartialReflect>, String> {
        (self.add)(assets, value)
    }

    /// a labeled asset of the file being loaded: `file#label`
    pub fn add_labeled(&self, load_context: &mut LoadContext, label: String, value: &dyn PartialReflect) -> Result<UntypedHandle, String> {
        (self.add_labeled)(load_context, label, value)
    }

    /// the handle `add_labeled` will fill, before the asset is made
    pub fn label_handle(&self, load_context: &mut LoadContext, label: String) -> UntypedHandle {
        (self.label_handle)(load_context, label)
    }
}

/// how a path-less asset reads: `asset:` and its index bits or uuid
fn asset_id_key(id: UntypedAssetId) -> String {
    match id {
        UntypedAssetId::Index { index, .. } => format!("asset:{}", index.to_bits()),
        UntypedAssetId::Uuid { uuid, .. } => format!("asset:{uuid}"),
    }
}

/// the asset of type `type_id` an `asset:` key names, None for anything else (a path)
pub fn parse_asset_id(key: &str, type_id: TypeId) -> Option<UntypedAssetId> {
    let id = key.strip_prefix("asset:")?;
    Some(match id.parse::<u64>() {
        Ok(bits) => UntypedAssetId::Index { type_id, index: AssetIndex::from_bits(bits) },
        Err(_) => UntypedAssetId::Uuid { type_id, uuid: Uuid::parse_str(id).ok()? },
    })
}

/// None if `value` is not a registered handle, else what it reads as
pub fn handle_key(value: &dyn PartialReflect, registry: &TypeRegistry) -> Option<String> {
    let info = value.get_represented_type_info()?;
    registry.get_type_data::<ReflectAssetPath>(info.type_id())?.key(value)
}

/// is this value a handle that's written as a path
pub fn is_handle(value: &dyn PartialReflect, registry: &TypeRegistry) -> bool {
    handle_key(value, registry).is_some()
}
