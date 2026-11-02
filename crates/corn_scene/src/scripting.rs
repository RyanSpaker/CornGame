//! CLAUDE (claude-opus-5-5): scene files' event handlers at runtime. A built scene holds its `on` items as
//! [`SceneListeners`] (inert while the scene is an asset); once spawned, each becomes a script listener
//! on its entity, calling into the file's module (loaded when the file was built).

use bevy::prelude::*;
use corn_scripting::{ListenerOf, ScriptListener};

use crate::SceneListeners;

pub(crate) fn plugin(app: &mut App) {
    app.add_observer(attach);
}

fn attach(add: On<Add<SceneListeners>>, listeners: Query<&SceneListeners>, mut commands: Commands) {
    let entity = add.event().entity;
    let Ok(listeners) = listeners.get(entity) else { return };
    for l in &listeners.0 {
        commands.spawn((ScriptListener::new(l.event.clone(), l.module.clone(), l.function.clone()), ListenerOf(entity)));
    }
}
