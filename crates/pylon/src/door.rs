//! CLAUDE (claude-opus-5-5): sliding doors. A [`DoorState`] holds the whole transition; scripts start it
//! by setting `state` to `Opening` or `Closing` (`component(door, "DoorState").state = "Opening"`), and
//! it settles at `Open` or `Closed`. The door's [`DoorLeaf`] children slide along its local X.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.register_type::<DoorState>().register_type::<DoorLeaf>().add_systems(Update, slide);
}

#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct DoorState {
    pub state: DoorPhase,
    /// 0 closed, 1 open
    pub progress: f32,
    /// seconds from closed to open
    pub duration: f32,
    /// how far each leaf moves when open
    pub travel: f32,
}

impl Default for DoorState {
    fn default() -> Self {
        Self { state: DoorPhase::Closed, progress: 0.0, duration: 0.65, travel: 0.4 }
    }
}

#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DoorPhase {
    #[default]
    Closed,
    Opening,
    Open,
    Closing,
}

/// a leaf of the parent's door. `side`: -1 slides to local -X, 1 to +X. With `clip` (and its `width`),
/// it's cut short (scaled in X) where it would reach past `clip` from the door's centre: a door in a wall
/// too narrow to take it
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct DoorLeaf {
    pub side: f32,
    pub width: f32,
    /// 0: never cut
    pub clip: f32,
}

/// where a leaf sits when closed, taken the first time it's seen
#[derive(Component)]
struct ClosedAt(Vec3);

fn slide(
    mut commands: Commands,
    time: Res<Time>,
    mut doors: Query<(&mut DoorState, &Children)>,
    mut leaves: Query<(Entity, &DoorLeaf, &mut Transform, Option<&ClosedAt>)>,
) {
    let dt = time.delta_secs();
    for (mut door, children) in &mut doors {
        let step = dt / door.duration.max(0.01);
        let (progress, state) = match door.state {
            DoorPhase::Opening if door.progress + step >= 1.0 => (1.0, DoorPhase::Open),
            DoorPhase::Opening => (door.progress + step, DoorPhase::Opening),
            DoorPhase::Closing if door.progress - step <= 0.0 => (0.0, DoorPhase::Closed),
            DoorPhase::Closing => (door.progress - step, DoorPhase::Closing),
            settled => (door.progress, settled),
        };
        if (progress, state) != (door.progress, door.state) {
            door.progress = progress;
            door.state = state;
        }
        let eased = progress * progress * (3.0 - 2.0 * progress);
        for &child in children {
            let Ok((leaf, side, mut transform, closed)) = leaves.get_mut(child) else { continue };
            let closed = match closed {
                Some(closed) => closed.0,
                None => {
                    commands.entity(leaf).insert(ClosedAt(transform.translation));
                    transform.translation
                }
            };
            let mut at = closed + Vec3::X * side.side * door.travel * eased;
            let mut scale = Vec3::ONE;
            if side.clip > 0.0 && side.width > 0.0 {
                // its span out from the centre, cut at `clip`
                let (inner, outer) = (at.x.abs() - side.width / 2.0, (at.x.abs() + side.width / 2.0).min(side.clip));
                let shown = (outer - inner).max(0.001);
                at.x = side.side * (inner + outer) / 2.0;
                scale.x = shown / side.width;
            }
            if transform.translation != at {
                transform.translation = at;
            }
            if transform.scale != scale {
                transform.scale = scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn opens_and_closes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, plugin));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        let door = app.world_mut().spawn(DoorState { duration: 0.5, travel: 1.0, ..default() }).id();
        let leaf = app.world_mut().spawn((DoorLeaf { side: 1.0, ..default() }, Transform::from_xyz(2.0, 0.0, 0.0), ChildOf(door))).id();
        app.update();
        app.world_mut().get_mut::<DoorState>(door).unwrap().state = DoorPhase::Opening;
        for _ in 0..8 {
            app.update();
        }
        let state = app.world().get::<DoorState>(door).unwrap();
        assert_eq!((state.state, state.progress), (DoorPhase::Open, 1.0));
        assert_eq!(app.world().get::<Transform>(leaf).unwrap().translation.x, 3.0);

        // turning back halfway
        app.world_mut().get_mut::<DoorState>(door).unwrap().state = DoorPhase::Closing;
        app.update();
        app.update();
        app.world_mut().get_mut::<DoorState>(door).unwrap().state = DoorPhase::Opening;
        app.update();
        assert_eq!(app.world().get::<DoorState>(door).unwrap().state, DoorPhase::Opening);
        app.world_mut().get_mut::<DoorState>(door).unwrap().state = DoorPhase::Closing;
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(app.world().get::<DoorState>(door).unwrap().state, DoorPhase::Closed);
        assert_eq!(app.world().get::<Transform>(leaf).unwrap().translation.x, 2.0);
    }

    // CLAUDE
    /// a clipped leaf never reaches past `clip`: it narrows instead
    #[test]
    fn clipped_leaves_stay_inside() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, plugin));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        let door = app.world_mut().spawn(DoorState { duration: 0.5, travel: 0.4, ..default() }).id();
        // 0.4 wide, closed over 0..0.4, in a wall reaching to 0.6
        let leaf = app.world_mut().spawn((DoorLeaf { side: 1.0, width: 0.4, clip: 0.6 }, Transform::from_xyz(0.2, 0.0, 0.0), ChildOf(door))).id();
        app.update();
        app.world_mut().get_mut::<DoorState>(door).unwrap().state = DoorPhase::Opening;
        for _ in 0..8 {
            app.update();
            let t = app.world().get::<Transform>(leaf).unwrap();
            assert!(t.translation.x + 0.2 * t.scale.x <= 0.6 + 1e-5, "{t:?}");
        }
        // open: 0.4..0.6 shows
        let t = app.world().get::<Transform>(leaf).unwrap();
        assert!((t.translation.x - 0.5).abs() < 1e-5 && (t.scale.x - 0.5).abs() < 1e-5, "{t:?}");
    }
}
