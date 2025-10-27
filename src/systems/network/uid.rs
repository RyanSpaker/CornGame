use std::{collections::HashSet, hash::Hasher as _};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use wgpu::naga::back::spv::DebugInfo;

pub struct UidPlugin;
impl Plugin for UidPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Uid>();
        app.register_type::<UidGen>();
        app.register_type::<UidDebug>();

        app.add_systems(PostUpdate, Uid::generate);
    }
}

/// predicatable Id which is the same on client and server, and unique
/// is a hierarchical hash, so we can, for example, salt the root of a blueprint to disambiguate multiple instances
/// should be immutable
#[derive(Debug, Copy, Clone, Component, Reflect, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Uid(pub u64);

impl Uid {
    pub fn map_entities() {

    }

    pub fn generate(
        // trigger: Trigger<OnAdd, UidGen> needs to run after SceneSpawned
        query: Query<(Entity, &UidGen), Without<Uid>>,
        parents: Query<&ChildOf>,
        uids: Query<&Uid>,
        names: Query<&Name>,
        uid_gen: Query<&UidGen>,
        mut commands: Commands,
    ) {
        let processed : HashSet<Entity> = HashSet::new();

        for (entity, uidgen) in query.iter() {
            // we have to process the tree from top to bottom, avoid reprocessing uids after that.
            if processed.contains(&entity) {
                continue;
            }

            // XXX currently Uid not allowed to change
            let root = parents.iter_ancestors(entity).find(|e| uids.contains(*e));
            let mut prev = root;

            let mut tree: Vec<_> = parents
                .iter_ancestors(entity)
                .take_while(|e| Some(*e) != root)
                .collect();
            tree.reverse();
            tree.push(entity);

            let mut prev_uid = root.map(|e| uids.get(e).unwrap().0 );

            let mut path: Vec<Option<String>> = Vec::new();
            let mut debug_str = String::new();

            // generate needed id's starting with furthest ancestor, since each id uses ancestors for id
            // TODO: generate in such a way that intermediate paths can be ignored
            // TODO: a way to create asset refs which are convertable to Uids
            for entity in tree {
                let name = names.get(entity).map(|n| n.as_str().to_string()).ok();
                path.push(name);

                let do_gen = uid_gen.contains(entity);
                if do_gen {
                    let mut debug = UidDebug::default();
                    debug.parent = prev;

                    let uid = uidgen.generate(&path, prev_uid, &mut debug);

                    commands.entity(entity).insert(Uid(uid));

                    prev = Some(entity);
                    prev_uid = Some(uid);
                    path.clear();
                }
            }
        }
    }
}

/// marker struck for the Uid generator system
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct UidGen {
    use_path: Option<UidUsePath>,
    seed: Option<String>,
    hierarchical: bool,
}

impl Default for UidGen {
    fn default() -> Self {
        Self {
            use_path: Some(UidUsePath::Name),
            seed: None,
            hierarchical: true,
        }
    }
}

impl UidGen {
    pub fn manual(seed: String) -> Self {
        Self {
            use_path: None,
            seed: Some(seed),
            hierarchical: false,
        }
    }

    fn generate(&self, path: &Vec<Option<String>>, prev: Option<u64>, debug: &mut UidDebug) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        if let Some(uid) = prev {
            if self.hierarchical {
                hasher.write_u64(uid);
                debug.data += &format!("{} ", uid);
            }
        }

        if let Some(seed) = &self.seed {
            hasher.write(seed.as_bytes());
            debug.data += &format!("{} ", seed);
        }

        if let Some(use_path) = self.use_path {
            let path = match use_path {
                UidUsePath::Path => {
                    let path : Vec<String> = path.iter().map(|a| a.as_ref().cloned().unwrap_or_default()).collect();
                    path.join("/")
                }
                UidUsePath::Name => {
                    path.last().cloned().flatten().unwrap_or_default().to_string()
                }
            };
            
            hasher.write(path.as_bytes());
            debug.data += &format!("{} ", path);
        }
        debug.data += "\n";
        hasher.finish()
    }
}

#[derive(Debug, Copy, Clone, Reflect, Default)]
pub enum UidUsePath {
    Path,
    #[default]
    Name,
}

/// stores debug info on how the Uid was generated
#[derive(Debug, Clone, Component, Reflect, Default)]
#[reflect(Component)]
pub struct UidDebug {
    data: String,
    parent: Option<Entity>,
}

// note: https://github.com/cBournhonesque/lightyear/blob/2037d468f513569deee79ca24e0eb06c2a4c35ea/examples/distributed_authority/src/server.rs#L58C1-L79C2
