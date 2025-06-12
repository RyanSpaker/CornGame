
use std::env;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_editor_pls::controls::{self, EditorControls};
use bevy_editor_pls::editor_window::{EditorWindow, EditorWindowContext};
use bevy_editor_pls::{egui, spawn_default_windows, AddEditorWindow, EditorPlugin, EguiPlugin};
use parking_lot::RwLock;

use crate::scenes::LoadScene;

fn eguibad<T: Send + Sync + Default + 'static>(ui: &mut egui::Ui, id: egui::Id) -> Arc<RwLock<T>>{
    ui.ctx().data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<RwLock<T>>>(id, Default::default).clone())
}

#[derive(Debug, Clone, Default, Component)]
struct SceneLoadWindow;

impl EditorWindow for SceneLoadWindow {
    fn ui(&self, world: &mut World, _cx: EditorWindowContext, ui: &mut egui::Ui) {
        // TODO tab complete
        let buffer = eguibad::<String>(ui, ui.auto_id_with("path"));
        ui.horizontal(|ui| {
            ui.label("Scene to load:");
            ui.text_edit_singleline(&mut *buffer.write());
        });

        // Add a button to trigger scene loading
        if ui.button("Load Scene").clicked() {
            world.spawn(LoadScene::new(&**buffer.read()));
        }
    }
}

#[derive(Debug)]
pub struct MyEditorPlugin;
impl Plugin for MyEditorPlugin{
    fn build(&self, app: &mut App) {
        if env::var("CORN_EDITOR") != Ok("0".to_string()) {
            app.add_plugins(EguiPlugin{
                enable_multipass_for_primary_context: false,
            });
            app.add_plugins(EditorPlugin::default());
            app.insert_resource(editor_controls());
            app.add_editor_window::<SceneLoadWindow>();
            app.add_systems(Startup, spawn_default_windows);
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
