use std::error::Error;

use bevy::{ecs::reflect, prelude::*};
use bevy_console::{reply, AddConsoleCommand, ConsoleCommand, ConsolePlugin};
use avian3d::{debug_render, prelude::*};
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::app::loading::TestBox;

pub struct MyConsolePlugin;
impl Plugin for MyConsolePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(ConsolePlugin)
            .add_console_command::<EchoCommand, _>(echo_command)
            .add_console_command::<SpawnCommand, _>(spawn_command)
            .add_console_command::<DebugCommand, _>(debug_command)

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
            SpawnCommand::TestCube => commands.spawn(TestBox),
        };
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
    }
}