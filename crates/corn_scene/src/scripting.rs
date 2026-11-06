//! CLAUDE (claude-opus-5-5): scene files' event handlers at runtime. A built scene holds its `on` items as
//! [`SceneListeners`] (inert while the scene is an asset); once spawned, each becomes a script listener
//! on its entity, calling into the file's module (loaded when the file was built).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use corn_scripting::{ListenerOf, ScriptEventsExt, ScriptListener};

use crate::doc::{self, Selector};
use crate::{build, names, Ready, SceneListeners};

pub(crate) fn plugin(app: &mut App) {
    app.add_observer(attach).register_script_entity_event::<Ready>();
    corn_scripting::set_select(select);
}

// CLAUDE
/// scripts' `select(e, "selector")`. selectors parse once per string; names resolve in the default crate
/// precedence, without a file's `use` lines. `#name` names what the calling script's file spawned
fn select(world: &World, scope: Entity, src: &str, module: Option<&str>) -> Result<Vec<Entity>, String> {
    static PARSED: Mutex<Option<HashMap<String, Arc<Vec<Selector>>>>> = Mutex::new(None);
    let registry = world.resource::<AppTypeRegistry>().read();
    let mut parsed = PARSED.lock().unwrap();
    let selectors = match parsed.get_or_insert_default().get(src) {
        Some(s) => s.clone(),
        None => {
            let index = names::TypeIndex::new(&registry);
            let precedence = names::parse_precedence("", index.crates());
            let s = Arc::new(doc::parse_selectors(src, names::Resolver::new(&index, &precedence)).map_err(|e| format!("select(\"{src}\"): {e}"))?);
            parsed.get_or_insert_default().insert(src.to_string(), s.clone());
            s
        }
    };
    drop(parsed);
    // a scene file's module is `path#$script`
    let file = module.and_then(|m| m.strip_suffix(&format!("#{}", crate::SCRIPT_LABEL)));
    Ok(build::select_spawned(world, &registry, scope, &selectors, file))
}

fn attach(add: On<Add<SceneListeners>>, listeners: Query<&SceneListeners>, mut commands: Commands) {
    let entity = add.event().entity;
    let Ok(listeners) = listeners.get(entity) else { return };
    for l in &listeners.0 {
        let listener = ScriptListener::new(l.event.clone(), l.module.clone(), l.function.clone()).with_component(l.component.clone());
        commands.spawn((listener, ListenerOf(entity)));
    }
}
