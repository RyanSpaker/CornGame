use bevy::prelude::*;
use bevy_console::{reply, AddConsoleCommand, ConsoleCommand};
use clap::Parser;

struct MyConsolePlugin;
impl Plugin for MyConsolePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(MyConsolePlugin)
            .add_console_command::<EchoCommand, _>(log_command)
            .run();
    }
}




/// Prints given arguments to the console
#[derive(Parser, ConsoleCommand)]
#[command(name = "echo")]
struct EchoCommand {
    /// Message to print
    msg: String,
}

fn log_command(mut command: ConsoleCommand<EchoCommand>) {
    if let Some(Ok(cmd)) = command.take() {
        let msg = cmd.msg;
        reply!(log, "{msg}");

        command.ok();
    }
}