/// This file contains the top level client-server replicated game lifecycle data + logic
/// This includes what levels / scenes are spawned. and the players.
/// 
/// Top level logic that is unique to the client (pause menu / main menu lifecycle, settings) does not belong here.

use std::collections::HashMap;

use bevy::{ecs::reflect::{ReflectBundle, ReflectCommandExt}, prelude::*, reflect::{serde::ReflectDeserializer, TypeRegistry}, scene::ron};
use lightyear::prelude::{Connected, Server};
use serde::{Deserialize, Serialize};

use crate::{ecs::menu_main::DefaultFloor, scenes::LoadScene, systems::network::uid::UidGen};

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // Register components/types used by replication system so reflect/serde is available.
        app.register_type::<Level>();
        app.register_type::<Game>();

        // Register components for replication (these hooks integrate with the project's
        // networking plugin conventions in `systems::network`).
        use lightyear::prelude::*;
        app.register_component::<Game>();
        app.add_systems(Update, sync_levels);
        app.add_observer(Game::on_swap_level);
        app.add_trigger::<SwapLevel>();

        // Optionally add game-level systems here.
        // app.add_systems(Update, my_game_system);
    }
}

type LevelId = String;
type SceneId = String;

/// Root game state structure. Registered for replication.
#[derive(Debug, Clone, Reflect, Component, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Game {
    pub loaded_levels: Vec<Level>,
}

impl Game {
    // **temp** code to spawn lobby on server
    pub(crate) fn lobby() -> Self {
        let path = "scenes/lobby.glb";
        Self {
            loaded_levels: vec![
                Level{
                    id: "lobby".to_string(),
                    scenes: HashMap::from([
                        (path.to_string(),path.to_string()),
                        ("default floor".to_string(), DefaultFloor::type_path().to_string() )] // testing out reflection strings, we want to do a cli thing
                    ),
                }
            ],
        }
    }

    // TODO disable physics during load
    fn on_swap_level(
        trigger: Trigger<SwapLevel>,
        mut game: Single<&mut Game>,
        _: Single<(&Server, &lightyear::prelude::server::Started)> // only run if server
    ){
        info!("{:?}", trigger.level);
        game.loaded_levels.clear();
        game.loaded_levels.push(trigger.level.clone());
    }
}


#[derive(Debug, Clone, Reflect, PartialEq, Serialize, Deserialize)]
#[reflect()]
pub struct Level {
    pub id: LevelId,
    pub scenes: HashMap<SceneId, String>,
}

/// indicates this Scene is part of Game
#[derive(Debug, Clone, Reflect, Component, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SceneMetadata {
    pub level: LevelId,
    pub scene: SceneId,
}


// #[derive(Debug, Clone, Reflect, Component, Serialize, Deserialize)]
// #[reflect(Component)]
// pub struct GamePlayerState {
//     pub alive: bool,
//     pub lobby: bool,
// }
    
// spawn / despawn levels based on what's in Game
// NOTE: lots of ways we could do this instead. This is simplest... for now
// alt: do it all with triggers/messages (downside is need to catch all edge cases)
// alt: use triggers to despawn, sync to spawn (downside is need to make sure cleared before connecting)
// alt: make Game data more component based (ie. Levels get their own entities.) This would be simpler for keeping things synced.
//      downside is we lose the simplicity of a single core Game data struct.
//      regardless this is likely the better approach
fn sync_levels(
    game: Single<&Game, Changed<Game>>,
    scenes: Query<(Entity, &SceneMetadata)>,
    type_registry: Res<AppTypeRegistry>,
    mut commands: Commands,
) {
    // Build desired set: map -> scene asset path
    let mut desired: HashMap<(LevelId, SceneId), String> = HashMap::new();
    for level in &game.loaded_levels {
        for (scene_id, scene_path) in &level.scenes {
            desired.insert((level.id.clone(), scene_id.clone()), scene_path.clone());
        }
    }

    // Iterate existing scene entities, despawn those not in desired, and record existing ones
    let mut existing = std::collections::HashSet::new();
    for (entity, meta) in scenes.iter() {
        let key = (meta.level.clone(), meta.scene.clone());
        if !desired.contains_key(&key) {
            commands.entity(entity).despawn();
        } else {
            existing.insert(key);
        }
    }

    // Spawn any desired scenes that don't yet exist
    for (key, path) in desired {
        if !existing.contains(&key) {
            let mut spawned = commands.spawn((
                SceneMetadata {
                    level: key.0.clone(),
                    scene: key.1.clone(),
                },
                UidGen::manual(format!("{}:{}", key.0, key.1))
            ));

            if path.ends_with("glb") || path.ends_with("gltf") {
                spawned.insert(LoadScene::new(&*path));
            }else{
                let path = format!("{{ \"{}\": () }}", path); // ron is a bit annoying

                let typs = type_registry.read();
                match string_to_component(&*typs, path){
                    Ok(v) => match v {
                        DynamicRef::Component(partial_reflect) => {
                            spawned.insert_reflect(partial_reflect);
                        },
                        DynamicRef::Bundle(partial_reflect) => todo!(),
                    },
                    Err(e) => error!("{}", e),
                }
            }
        }
    }
}

pub enum DynamicRef {
    Component(Box<dyn PartialReflect>),
    Bundle(Box<dyn PartialReflect>),
}

/// helper function to convert ron_string to component
fn string_to_component(
    type_registry: &TypeRegistry,
    ron_string: String,
) -> Result<DynamicRef> {
    use serde::{Serialize, Deserialize, de::DeserializeSeed};


    let reflect_serializer = bevy::reflect::serde::ReflectSerializer::new(&DefaultFloor, &type_registry);
    let output = ron::to_string(&reflect_serializer).unwrap();
    dbg!(output);

    let reflect_deserializer = ReflectDeserializer::new(&type_registry);
    let mut deserializer = ron::de::Deserializer::from_str(&ron_string)?;
    let reflect_value =
        reflect_deserializer.deserialize(&mut deserializer)?;

    // FROM https://docs.rs/bevy_ecs/0.17.2/src/bevy_ecs/reflect/entity_commands.rs.html#312
    let type_info = reflect_value
        .get_represented_type_info()
        .expect("component should represent a type.");
    let type_path = type_info.type_path();
    let Some(type_registration) = type_registry.get(type_info.type_id()) else {
        return Err(format!("`{type_path}` should be registered in type registry via `App::register_type<{type_path}>`").into());
    };

    if let Some(reflect_component) = type_registration.data::<ReflectComponent>() {
        return Ok( DynamicRef::Component(reflect_value) )
    } else if let Some(reflect_bundle) = type_registration.data::<ReflectBundle>() {
        return Ok( DynamicRef::Bundle(reflect_value) )
    } else {
        return Err(format!("`{type_path}` should have #[reflect(Component)] or #[reflect(Bundle)]").into());
    }
}



#[derive(Debug, Clone, Event, Reflect, PartialEq, Serialize, Deserialize)]
pub struct SwapLevel {
    pub level: Level
}
