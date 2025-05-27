use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::sync_world::RenderEntity;

pub struct DebugWarnRenderId;
impl Plugin for DebugWarnRenderId {
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_systems(Startup, crank_render_generations);
        app.add_systems(Update, warn_synced_ids);
    }
}

/// deliberately desync render world and main world Entities to catch migration bugs faster.
/// XXX have not been able to confirm this makes any difference
fn crank_render_generations(world: &mut World) {
    let v: Vec<_> = (0..100).map(|_| world.spawn(()).id()).collect();
    for e in v {
        world.despawn(e);
    }
}

fn warn_synced_ids(query: Query<(Entity, &RenderEntity), Changed<RenderEntity>>) {
    for (id, r_id) in query.iter() {
        if id == r_id.id() {
            warn!("render world id's sync'd for {} don't rely on this", id);
        }
    }
}
