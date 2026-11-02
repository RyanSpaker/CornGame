//! CLAUDE (claude-opus-5-5): events scripts listen to and trigger. Always rust types: registering one adds
//! a bridge observer that queues calls for its [`ScriptListener`]s, and lets scripts trigger it.
//! An entity event's target is the one its listeners on entities hear; a script triggering one gives the
//! target as `trigger`'s first argument, which fills the event's one `Entity` field.
//! Lifecycle events (`AddEvent`, `InsertEvent`, `RemoveEvent`, also named `OnAdd`, `OnInsert`, `OnRemove`)
//! are per component: a listener names the component, and the first listener for a pair spawns its
//! bridge. Scripts can't trigger them.

use std::any::TypeId;

use bevy::{
    ecs::{component::ComponentId, system::SystemParam},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    reflect::{serde::TypedReflectSerializer, GetTypeRegistration, TypeInfo, TypeRegistry, Typed},
};

use super::{deserialize, entity_key, live, lookup, values::ScriptValues, Call, ListenerOf, ScriptListener, ScriptListeners, ScriptQueue};

pub trait ScriptEventsExt {
    /// let scripts listen to and trigger `E`, an event without a target
    fn register_script_event<E>(&mut self) -> &mut Self
    where
        E: Event + FromReflect + GetTypeRegistration + Typed,
        for<'a> E::Trigger<'a>: Default;

    /// let scripts listen to and trigger `E`, an event with a target entity
    fn register_script_entity_event<E>(&mut self) -> &mut Self
    where
        E: EntityEvent + FromReflect + GetTypeRegistration + Typed,
        for<'a> E::Trigger<'a>: Default;
}

impl ScriptEventsExt for App {
    fn register_script_event<E>(&mut self) -> &mut Self
    where
        E: Event + FromReflect + GetTypeRegistration + Typed,
        for<'a> E::Trigger<'a>: Default,
    {
        register::<E>(self, trigger_typed::<E>);
        self.add_observer(|on: On<E>, mut listeners: Listeners| listeners.queue(on.event(), None, None))
    }

    fn register_script_entity_event<E>(&mut self) -> &mut Self
    where
        E: EntityEvent + FromReflect + GetTypeRegistration + Typed,
        for<'a> E::Trigger<'a>: Default,
    {
        register::<E>(self, trigger_typed::<E>);
        self.add_observer(|on: On<E>, mut listeners: Listeners| {
            let event = on.event();
            listeners.queue(event, Some(event.event_target()), None)
        })
    }
}

fn register<E: Event + GetTypeRegistration>(app: &mut App, trigger: TriggerFn) {
    app.register_type::<E>();
    let mut registry = app.world_mut().get_resource_or_init::<ScriptEventRegistry>();
    registry.types.insert(TypeId::of::<E>(), EventType { trigger: Some(trigger), observe: None });
}

/// let scripts listen to a component lifecycle event
pub(super) fn register_lifecycle<E: EntityEvent + Reflect + GetTypeRegistration + Typed>(app: &mut App, alias: &'static str) {
    app.register_type::<E>();
    let mut registry = app.world_mut().get_resource_or_init::<ScriptEventRegistry>();
    registry.types.insert(TypeId::of::<E>(), EventType { trigger: None, observe: Some(observe_component::<E>) });
    registry.aliases.insert(alias, TypeId::of::<E>());
}

type TriggerFn = fn(&mut World, Option<Entity>, &serde_json::Value, &TypeRegistry, Option<&AssetServer>) -> Result<(), String>;

#[derive(Default, Resource)]
pub(super) struct ScriptEventRegistry {
    types: HashMap<TypeId, EventType>,
    /// other names for events: `OnAdd`
    aliases: HashMap<&'static str, TypeId>,
    /// lifecycle events with a bridge, by component
    observed: HashSet<(TypeId, ComponentId)>,
}

struct EventType {
    /// None for lifecycle events
    trigger: Option<TriggerFn>,
    /// lifecycle events: spawn a bridge for the component
    observe: Option<fn(&mut World, ComponentId)>,
}

/// a listener's event, resolved from its names
#[derive(Component)]
pub(super) struct ListenerEvent {
    event: TypeId,
    component: Option<ComponentId>,
}

/// a registered event's type, by full or short type path
fn resolve(registry: &TypeRegistry, events: &ScriptEventRegistry, name: &str) -> Result<TypeId, String> {
    if let Some(&id) = events.aliases.get(name) {
        return Ok(id);
    }
    let registration = registry
        .get_with_type_path(name)
        .or_else(|| registry.get_with_short_type_path(name))
        .ok_or_else(|| format!("unknown type `{name}` (not registered, or an ambiguous short name)"))?;
    let id = registration.type_id();
    events
        .types
        .contains_key(&id)
        .then_some(id)
        .ok_or_else(|| format!("`{name}` is not a script event (see register_script_event)"))
}

// CLAUDE
/// resolve a listener's event (and component), spawning the bridge a lifecycle event needs
pub(super) fn resolve_listener(world: &mut World, listener: &ScriptListener) -> Result<ListenerEvent, String> {
    let registry = super::type_registry(world);
    let registry = registry.read();
    let event = resolve(&registry, world.resource(), &listener.event)?;
    let observe = world.resource::<ScriptEventRegistry>().types[&event].observe;
    let component = match (observe, listener.component.as_str()) {
        (None, "") => None,
        (None, _) => return Err(format!("`{}` is not a lifecycle event, it takes no component", listener.event)),
        (Some(_), "") => return Err(format!("`{}` needs a component", listener.event)),
        (Some(observe), name) => {
            let id = lookup(&registry, name)?.1.register_component(world);
            if world.resource_mut::<ScriptEventRegistry>().observed.insert((event, id)) {
                observe(world, id);
            }
            Some(id)
        }
    };
    Ok(ListenerEvent { event, component })
}

fn observe_component<E: EntityEvent + Reflect + TypePath>(world: &mut World, component: ComponentId) {
    let bridge = move |on: On<E>, mut listeners: Listeners| {
        let event = on.event();
        listeners.queue(event, Some(event.event_target()), Some(component))
    };
    world.spawn(Observer::new(bridge).with_component(component));
}

#[derive(SystemParam)]
struct Listeners<'w, 's> {
    on_entities: Query<'w, 's, &'static ScriptListeners>,
    listeners: Query<'w, 's, (&'static ScriptListener, &'static ListenerEvent)>,
    everywhere: Query<'w, 's, (Entity, &'static ScriptListener, &'static ListenerEvent), Without<ListenerOf>>,
    registry: Res<'w, AppTypeRegistry>,
    queue: ResMut<'w, ScriptQueue>,
}

impl Listeners<'_, '_> {
    // CLAUDE
    /// queue a call for each listener on the target, and each listener on every entity
    fn queue<E: Event + Reflect + TypePath>(&mut self, fired: &E, target: Option<Entity>, component: Option<ComponentId>) {
        let on_target = target.and_then(|t| self.on_entities.get(t).ok()).into_iter().flat_map(|l| l.iter());
        let on_target = on_target.filter_map(|l| self.listeners.get(l).ok().map(|(s, e)| (l, s, e)));
        let mut payload = None;
        for (listener, script, event) in on_target.chain(self.everywhere.iter()) {
            if event.event != TypeId::of::<E>() || event.component != component {
                continue;
            }
            let payload = payload.get_or_insert_with(|| {
                let registry = self.registry.read();
                let processor = ScriptValues(None);
                let event = TypedReflectSerializer::with_processor(fired.as_partial_reflect(), &registry, &processor);
                serde_json::to_string(&event).unwrap_or_else(|e| {
                    warn!("script event `{}` has no json form: {e}", E::type_path());
                    "null".into()
                })
            });
            self.queue.0.push(Call {
                module: script.module.clone(),
                function: script.function.clone(),
                owner: Some(listener),
                target,
                args: script.args.clone(),
                event: payload.clone(),
            });
        }
    }
}

// CLAUDE
/// trigger a registered event from script data. rust observers run before this returns
pub(super) fn trigger(world: &mut World, target: Option<Entity>, name: &str, data: &serde_json::Value) -> Result<(), String> {
    let registry = super::type_registry(world);
    let assets = super::asset_server(world);
    let registry = registry.read();
    let id = resolve(&registry, world.resource::<ScriptEventRegistry>(), name)?;
    let trigger = world.resource::<ScriptEventRegistry>().types[&id].trigger;
    let trigger = trigger.ok_or_else(|| format!("`{name}` is triggered by bevy, not scripts"))?;
    let data = if data.is_null() { &serde_json::json!({}) } else { data };
    // observers can change anything
    live::clear_cache();
    trigger(world, target, data, &registry, assets.as_ref())
}

// CLAUDE
fn trigger_typed<E>(world: &mut World, target: Option<Entity>, data: &serde_json::Value, registry: &TypeRegistry, assets: Option<&AssetServer>) -> Result<(), String>
where
    E: Event + FromReflect + Typed,
    for<'a> E::Trigger<'a>: Default,
{
    let mut data = data.clone();
    if let Some(target) = target {
        let field = target_field(E::type_info())
            .ok_or_else(|| format!("`{}` has no target: trigger it with `null` as the entity", E::type_path()))?;
        let object = data.as_object_mut().ok_or_else(|| format!("`{}` data must be an object", E::type_path()))?;
        object.insert(field.to_string(), entity_key(target).into());
    }
    let value = deserialize(registry.get(TypeId::of::<E>()).unwrap(), registry, assets, &data)?;
    let event = E::from_reflect(value.as_ref()).ok_or_else(|| format!("data doesn't fit `{}`", E::type_path()))?;
    world.trigger(event);
    Ok(())
}

/// an entity event's target field: its one `Entity` field
fn target_field(info: &TypeInfo) -> Option<&'static str> {
    let TypeInfo::Struct(s) = info else { return None };
    let mut entities = s.iter().filter(|f| f.type_id() == TypeId::of::<Entity>());
    let field = entities.next()?;
    entities.next().is_none().then(|| field.name())
}
