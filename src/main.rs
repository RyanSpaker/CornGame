use std::path::PathBuf;

use bevy::prelude::*;
use corn_game::{systems::scenes::{initial_scenes::InitialSceneExt, testing::TestRegisterExt}, CornGameAppAPI, DevConfig};
use we_clap::WeParser;

#[derive(Debug, Clone, clap::Parser, Default)]
struct Cli {
    #[arg(long)]
    headless: bool,
    #[arg(short, long)]
    server: bool,
    #[arg(short, long)]
    client: bool,

    #[arg(long)]
    dummy: bool,

    #[arg(long)]
    no_vsync: bool,

    #[arg(long)]
    test: Option<String>,

    #[arg(long)]
    menu: bool,
    #[arg(long)]
    lobby: bool,
    #[arg(long)]
    empty: bool,
    #[arg(long)]
    no_global: bool,

    #[arg(short, long)]
    list_scenes: bool,

    scenes: Vec<PathBuf>,
}
impl we_clap::WeParser for Cli {}
impl Into<DevConfig> for Cli{
    fn into(self) -> DevConfig {
        DevConfig { dummy: self.dummy, server: self.server, client: self.client, scenes: self.scenes }
    }
}


fn main() -> AppExit{
    let cli: Cli = Cli::we_parse();
    let mut app = App::new();
    // Parse CLI
    if cli.headless {
        app.setup_game_headless();
    } else {
        app.setup_game(!cli.no_vsync);
    }
    app.insert_dev_config(cli.clone().into());
    // List scenes
    if cli.list_scenes{
        app.finish();
        let scenes = app.get_scene_list();
        let tests = app.get_test_list();
        println!("Cached Embedded Scenes: {}", scenes.len());
        for scene in scenes {println!("\t{}", scene);}
        println!("\nRegistered Tests: {}", tests.len());
        for test in tests {println!("\t{}", test);}
        return AppExit::Success;
    }
    // Parse Initial Scenes
    if cli.menu {
        app.set_initial_scenes(vec!["embedded#main_menu".into()]);
    }else if cli.lobby || !cli.scenes.is_empty() {
        app.set_initial_scenes(vec!["embedded#lobby".into()]);
    } else if cli.empty{
        app.disable_default_main();
    }
    if cli.no_global {app.set_global_scene(false);}
    // Parse test cli
    if let Some(test) = cli.test{
        app.activate_test(test);
    }
    // Run
    app.run()
}
