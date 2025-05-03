
use std::env;

use bevy::prelude::*;
use bevy_editor_pls::controls::{self, EditorControls};
use bevy_editor_pls::EditorPlugin;

#[derive(Debug)]
pub struct MyEditorPlugin;
impl Plugin for MyEditorPlugin{
    fn build(&self, app: &mut App) {
        if env::var("CORN_EDITOR") != Ok("0".to_string()) {
            app.add_plugins(EditorPlugin::default());
            app.insert_resource(editor_controls());
        }

        app.add_systems(Startup, |mut window: Query<&mut Window>, cli: Res<crate::Cli>|{
            for mut w in window.iter_mut() {
                dbg!(w.resolution.scale_factor());
                dbg!(w.resolution.base_scale_factor());
                dbg!(w.resolution.scale_factor_override());

                w.resolution.set_scale_factor(1.5);
                if cli.client {
                    w.title = "client".into();
                }
                if cli.server {
                    w.title = "server".into();
                    if cli.client {
                        w.title = "client server".into()
                    }
                }
            }      
        });
    }
}

fn editor_controls() -> EditorControls {
    let mut editor_controls = EditorControls::default_bindings();
    editor_controls.unbind(controls::Action::PlayPauseEditor);

    editor_controls.insert(
        controls::Action::PlayPauseEditor,
        controls::Binding {
            input: controls::UserInput::Single(controls::Button::Keyboard(KeyCode::F3)),
            conditions: vec![controls::BindingCondition::ListeningForText(false)],
        },
    );

    editor_controls
}