use std::error::Error;

use bevy::{ecs::{query::QuerySingleError, reflect}, prelude::*};
use bevy_console::{reply, AddConsoleCommand, ConsoleCommand, ConsolePlugin};
use avian3d::{debug_render, prelude::*};
use blenvy::GameWorldTag;
use clap::Parser;
use lightyear::{connection::server::ServerConnections, prelude::{client::{self, ClientConnection, NetClient, NetworkingState}, server}};
use serde::{Deserialize, Serialize};
use wgpu::hal::auxil::db;

use crate::app::{character::SpawnPlayerEvent, loading::TestCube};

// TODO: 
// - commands to query and act on entities
// - generic way to trigger events
// - retained history
// - camera modes
// - integrate keybinds
// - command to add things to a live stat overlay
//    - ex) specific entities position
//    - ex) query filtered entity counts
// - retained selection
// - bevy mod picking integration?
// - command to open up floating editor windows, with component modification

pub struct MyConsolePlugin;
impl Plugin for MyConsolePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(ConsolePlugin)
            .add_console_command::<EchoCommand, _>(echo_command)
            .add_console_command::<SpawnCommand, _>(spawn_command)
            .add_console_command::<DebugCommand, _>(debug_command)
            .add_console_command::<RespawnCommand, _>(respawn_command)
            .add_console_command::<ReloadCommand, _>(reload_command)
            .add_console_command::<NetTest, _>(nettest_command)

            .register_type::<Initial<Transform>>()
            .add_console_command::<ResetCommand, _>(reset_command.before(PhysicsSet::Sync))
            .add_systems(PostUpdate, record_initial::<Transform>.before(PhysicsSet::Sync));
    }
}

/// Prints given arguments to the console
#[derive(Parser, ConsoleCommand)]
#[command(name = "echo")]
struct EchoCommand {
    /// Message to print
    msg: String,
}

fn echo_command(mut ctx: ConsoleCommand<EchoCommand>) {
    if let Some(Ok(cmd)) = ctx.take() {
        let msg = cmd.msg;
        reply!(ctx, "{msg}");

        ctx.ok();
    }
}

#[derive(Parser, ConsoleCommand)]
#[command(name = "spawn")]
enum SpawnCommand {
    TestCube
}

fn spawn_command(mut ctx: ConsoleCommand<SpawnCommand>, mut commands: Commands){
    if let Some(Ok(cmd)) = ctx.take() {
        match cmd {
            SpawnCommand::TestCube => commands.spawn(TestCube),
        };
    }
}

#[derive(Parser, ConsoleCommand)]
#[command(name = "respawn")]
struct RespawnCommand{
    target: Option<String>,   
}

fn respawn_command(mut ctx: ConsoleCommand<RespawnCommand>, mut commands: Commands){
    if let Some(Ok(cmd)) = ctx.take() {
        commands.trigger(SpawnPlayerEvent{target:cmd.target});
    }
}

#[derive(Parser, ConsoleCommand)]
#[command(name = "reload")]
struct ReloadCommand{
    path: Option<String>,
}

fn reload_command(
    mut ctx: ConsoleCommand<ReloadCommand>, 
    mut commands: Commands, 
    scene: Query<(Entity, &blenvy::BlueprintInfo), With<GameWorldTag>>
){
    if let Some(Ok(cmd)) = ctx.take() {
        match scene.get_single() {
            Ok((id, info)) => {
                let path = cmd.path.unwrap_or(info.path.clone());
                commands.entity(id).despawn();
                commands.spawn((
                    blenvy::BlueprintInfo::from_path(&path), 
                    blenvy::SpawnBlueprint,
                    blenvy::GameWorldTag,
                    RigidBody::Static // NOTE: keeping this function in sync with the one in loading/mod.rs is error prone. 
                                      // TODO: loading event
                ));
            },
            Err(QuerySingleError::NoEntities(_)) => {
                let Some(path) = cmd.path else {
                    ctx.reply_failed("must specify path");
                    return;
                };
                commands.spawn((
                    blenvy::BlueprintInfo::from_path(&path), 
                    blenvy::SpawnBlueprint,
                    blenvy::GameWorldTag
                ));
            },
            Err(QuerySingleError::MultipleEntities(_)) => {
                todo!()
            }
        }
    }
}

#[derive(Component, Reflect, Debug, Serialize, Deserialize, Clone)]
#[reflect(Component)]
pub struct Initial<M: Component + Clone>(pub M);
fn record_initial<M: Component + Clone>(query: Query<(Entity, &M), Added<M>>, mut commands: Commands){
    for (e,c) in query.iter(){
        commands.entity(e).insert(Initial(c.clone()));
    }
}

#[derive(Parser, ConsoleCommand)]
#[command(name = "reset")]
enum ResetCommand {
    All,
    Entity{
        entity: String
    }
}

#[auto_enums::auto_enum(Error)]
fn entity_from_string(s: String) -> Result<Entity, impl Error> {
    let (low, high) : (u32, u32) = prse::try_parse!(s, "{}v{}")?;
    let v = ((high as u64) << u32::BITS) | (low as u64);
    let e = Entity::try_from_bits(v)?;
    Ok(e)
}

fn reset_command(mut ctx: ConsoleCommand<ResetCommand>, mut commands: Commands, query : Query<(Entity, &Initial<Transform>)>){
    if let Some(Ok(cmd)) = ctx.take() {
        match cmd {
            ResetCommand::All => {
                /* TODO dont want to put query for all objects in this system which runs every time */
                for (e, initial) in query.iter() {
                    commands.entity(e).insert(initial.0);
                    commands.entity(e).insert((
                        LinearVelocity::default(),
                        AngularVelocity::default(),
                    ));
                }
            },
            ResetCommand::Entity{entity} => { 
                match entity_from_string(entity) {
                    Ok(e) => {
                        let Ok((_, initial)) = query.get(e) else { return; };
                        commands.entity(e).insert(initial.0);
                        commands.entity(e).insert((
                            LinearVelocity::default(),
                            AngularVelocity::default(),
                        ));
                    },
                    Err(e) => reply!(ctx, "{e}"),
                }
            }
        };
    }
}

#[derive(Parser, ConsoleCommand)]
#[command(name = "debug")]
enum DebugCommand {
    Physics
}

fn debug_command(mut ctx: ConsoleCommand<DebugCommand>, mut gizmo: ResMut<GizmoConfigStore>) {
    if let Some(Ok(cmd)) = ctx.take() {
        let (config, _) = gizmo.config_mut::<PhysicsGizmos>();
        config.enabled = !config.enabled;
        reply!(ctx, "{}", config.enabled);
        ctx.ok();
    }
}


#[derive(Parser, ConsoleCommand)]
#[command(name = "nettest")]
struct NetTest;

fn nettest_command(
    mut ctx: ConsoleCommand<NetTest>,
    client: Res<ClientConnection>,
    client_config: Res<client::ClientConfig>,
    server_config: Res<server::ServerConfig>,
    server: Res<ServerConnections>,
    server_state: Res<State<server::NetworkingState>>,
    client_state: Res<State<client::NetworkingState>>,
){
    if let Some(Ok(cmd)) = ctx.take() { // seriously fuck this api
        dbg!(&client_state);
        dbg!(&server_state);
        dbg!(&client_config.shared.mode);
        dbg!(&server_config.shared.mode);
        reply!(ctx, "server {:?}", server_state);
        reply!(ctx, "client {} {:?}", client.client.id(), client_state);
        ctx.ok();
    }
}