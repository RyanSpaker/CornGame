use std::hash::Hasher as _;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub struct UidPlugin;
impl Plugin for UidPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Uid>();
        app.register_type::<UidGen>();
        app.register_type::<UidUsePath>();
        app.register_type::<UidDebug>();
        app.register_type::<UidSeed>();
        
        app.add_observer(Uid::generate);
    }
}

/// predicatable Id which is the same on client and server, and unique
/// is a hierarchical hash, so we can, for example, salt the root of a blueprint to disambiguate multiple instances
/// should be immutable
#[derive(Debug, Copy, Clone, Component, Reflect, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Uid(pub u64);

impl Uid {
    pub fn map_entities() {}

    pub fn generate(
        trigger: Trigger<OnAdd, UidGen>,
        parents: Query<&ChildOf>,
        uids: Query<&Uid>,
        seeds: Query<&UidSeed>,
        names: Query<&Name>,
        use_path: Query<&UidUsePath>,
        uid_gen: Query<&UidGen>,
        mut commands: Commands,
    ) {
        let e = trigger.target();

        // XXX currently Uid not allowed to change
        let root = parents.iter_ancestors(e).find(|e| uids.contains(*e));

        let mut tree: Vec<_> = parents
            .iter_ancestors(e)
            .take_while(|e| Some(*e) != root)
            .collect();
        tree.reverse();
        tree.push(e);

        let mut uid = 0;
        if let Some(e) = root {
            uid = uids.get(e).unwrap().0;
        }

        let mut path: Vec<Option<&str>> = Vec::new();
        let mut debug_str = String::new();

        // generate needed id's starting with furthest ancestor, since each id uses ancestors for id
        // TODO: generate in such a way that intermediate paths can be ignored
        // TODO: a way to create asset refs which are convertable to Uids
        for entity in tree {
            let name = names.get(entity).map(|n| n.as_str()).ok();
            path.push(name);

            let do_gen = uid_gen.contains(entity);
            if do_gen {
                let mut hasher = std::hash::DefaultHasher::new();
                hasher.write_u64(uid);
                debug_str += &format!("{}\n", uid);

                if let Ok(use_path) = use_path.get(entity) {
                    match use_path {
                        UidUsePath::Path => {
                            for p in path.iter() {
                                if let Some(p) = p {
                                    hasher.write((*p).as_bytes());
                                    debug_str += &format!("{:?}\n", p);
                                }
                            }
                        }
                        UidUsePath::Name => {
                            if let Some(n) = name {
                                hasher.write((*n).as_bytes());
                                debug_str += &format!("{:?}\n", n);
                            }
                        }
                    }
                }

                if let Ok(seed) = seeds.get(entity) {
                    hasher.write(seed.0.as_bytes());
                    debug_str += &format!("{:?}\n", seed);
                }

                uid = hasher.finish();
                commands.entity(entity).insert(Uid(uid));

                path.clear();
            }
        }
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(UidGen)]
pub struct UidSeed(String);

#[derive(Debug, Copy, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(UidGen)]
pub enum UidUsePath {
    Name,
    Path,
}

#[derive(Debug, Copy, Clone, Component, Reflect, Default)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct UidGen;

#[derive(Debug, Clone, Component, Reflect, Default)]
#[reflect(Component)]
pub struct UidDebug(String);

// note: https://github.com/cBournhonesque/lightyear/blob/2037d468f513569deee79ca24e0eb06c2a4c35ea/examples/distributed_authority/src/server.rs#L58C1-L79C2