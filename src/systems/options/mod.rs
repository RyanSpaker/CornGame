use std::any::Any;
use bevy::{ecs::entity::{EntityHashMap, EntityHashSet}, prelude::*};
use serde::Serialize;

/*
    Options are Resources
    When necessary, entities are spawned, which clone their values into the resources
    The entities hold information about how the options are rendered and specified in the options menu

    Level A: Resource
    All options are resources.
    Generally they are treated as immutable, only being edited by specialized sub-systems such as the options menu
    Options are resources which implement Option. The trait implementation defines all meta info used by the options menu.
    Options generally are cmposed of fields that implement OptionAtomic, or Option. The resource can be thought of as a tree with all leaves being OptionAtomic
    Options may also have special fields which are treated as constant, and are only shown in the options menu for info.
    OptionAtomics are type with known and defined input pipelines.
    Derive macros will be used to make options extremely easy to implement and construct.

    Level B: Entity Heirarchy
    When necessary the app will construct an entitiy heirarchy from the resource to facilitate changes to options changing the app.
    Leaf entities will hold the actual fields of the option, and changes will be reported to the resource and written in.
    A system per OptionAtomic will run converting changed leaves into events holding (field path, dyn Any(value)).
    A system per option resource will run, and check all changed events for the ones coming from owned entities, and the update their values from those changes.
    When entities are created from a resource, all leaf entities resulting from the entity must be recorded somewhere for the per resource systems.

    While the leaves must correspond to fields of resources, the entitiy tree may not reflect the option resource tree.
    For instance, you might stick all options for flycam in a resource, keybinds and all, but want all keybinds to have their own tree.
    The leaves may also not be unique. For this reason, duplicate leaves need some connection which replicates changes between them. This will be tricky with change detection

    One screen of the options will probably just list all option resources as seperate groups, and the entity tree can be easily constructed from the resource

    The top level option heirarchy will be defined prior to app run, and is constant. an app extension will allow for defining custom nodes of the heirarchy.

    As an example this may be the default heirarchy:  
    Root:
    - Option Resources
    - - Each option in sequence with its own tree
    - General
    - Video
    - Audio
    - Controls
    - - Keybinds:
    - - All other movement config like sensitivity, or movement speed
    - Accessibility

    Option fields can be tagged with a macro to have them show up in a category, like #[part_of(Keybinds)]
*/

/// Tag component for all entities of the option heirarchy
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct OptionEntity;
/// Tag component for atomic option entities
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct AtomicOptionEntity;

/// Struct used to build the entitiy heirarchy of an option. 
pub struct OptionHierarchyBuilder<'w, 's>{
    commands: Commands<'w, 's>,
    entity_tree: EntityHashMap<OptionHierarchyNode>,
    atomics: EntityHashSet
}
impl<'w, 's> OptionHierarchyBuilder<'w, 's>{
    pub fn register(&mut self, name: String, parent: Entity) -> Entity{
        let id = self.commands.spawn((OptionEntity, Name::from(name.clone()))).set_parent(parent).id();
        self.entity_tree.insert(id, OptionHierarchyNode { name, children: vec![], parent });
        self.entity_tree.get_mut(&parent).unwrap().children.push(id);
        id
    }
    pub fn register_atomic<A: OptionAtomic>(&mut self, name: String, parent: Entity, data: A) -> Entity{
        let id = self.commands.spawn((
            OptionEntity, AtomicOptionEntity, Name::from(name.clone())
        )).insert(OptionAtomicData(data)).set_parent(parent).id();
        self.entity_tree.insert(id, OptionHierarchyNode { name, children: vec![], parent });
        self.entity_tree.get_mut(&parent).unwrap().children.push(id);
        self.atomics.insert(id);
        A::build_atomic_option(self, id);
        id
    }
}
pub struct OptionHierarchyNode{
    name: String,
    children: Vec<Entity>,
    parent: Entity
}

/// Trait implemented for all types that have ui input methods. Act as the leaves of Option Resources
pub trait OptionAtomic: Clone+Serialize+Sized+Send+Sync+'static{}
#[derive(Component)]
pub struct OptionAtomicData<A: OptionAtomic>(pub A);

/// Trait implemented for all Options of the App
pub trait OptionResource{
    /// Constructs the option heirarchy with this option
    fn build_option(&self, builder: &mut OptionHierarchyBuilder, parent: Entity);
}

pub struct TestOptions{
    toggle: bool, // "toggle,"
    float: f32, // "float,"
    int: i32, // "int,"
    uint: u32, // "uint,"
    key: KeyCode, // "key,"
    sub_option: TestSubOption // "sub_option,"
}

pub struct TestSubOption{
    float: f32 // "sub_option,float,"
}
impl OptionResource for TestSubOption{
    fn build_option(&self, builder: &mut OptionHierarchyBuilder, parent: Entity) {
        builder.register_atomic::<f32>("float".to_string(), parent, self.float);

    }
}

#[derive(Default, Debug, Clone)]
pub struct OptionUpdateError;
impl std::fmt::Display for OptionUpdateError{fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{:?}", Self))
}}
impl std::error::Error for OptionUpdateError{}

pub fn write_option_data<S: Any, T: Any>(target: &mut S, value: T) -> Result<(), OptionUpdateError>{
    let data: Box<dyn Any> = Box::new(value);
    let data = data.downcast::<S>().or(Err(OptionUpdateError))?;
    *target = *data;
    Ok(())
}

pub struct OptionsPlugin;
impl Plugin for OptionsPlugin{
    fn build(&self, app: &mut App) {

    }
}