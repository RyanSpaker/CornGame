/// support for renderdoc
/// TODO DELETEME not actually needed.
/// https://docs.rs/bevy-renderdoc-capture/0.2.0/src/bevy_renderdoc_capture/lib.rs.html#1-188

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ptr::null, sync::Mutex};

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSet};

type RenderDoc = renderdoc::RenderDoc<renderdoc::V100>;

#[derive(Resource)]
struct RenderDocData {
    capture_requested: Arc<AtomicBool>,
    is_capture_active: bool,
    api: Mutex<RenderDoc>,
}

/// The RenderDoc capture plugin.
pub struct RenderDocPlugin {
    key_code: Option<KeyCode>,
}

/// Resource for triggering a renderdoc capture using [RenderDocTrigger::capture].
///
/// Triggers are handled during [ExtractSchedule] and you should enable capture prior.
///
/// By default, a hook is registered that triggers a renderdoc capture upon pressing F10.
///
/// # Example
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_renderdoc_capture::*;
/// pub fn my_system(trigger: Res<RenderDocTrigger>) {
///
///     // Do some checks...
///
///     trigger.capture();
/// }
/// ```
#[derive(Resource)]
pub struct RenderDocTrigger {
    is_capture_active: Arc<AtomicBool>,
}

impl RenderDocTrigger {
    /// Trigger capturing the next frame for RenderDoc.
    ///
    /// This operation cannot be undone.
    pub fn capture(&self) {
        self.is_capture_active.store(true, Ordering::SeqCst);
    }
}

impl Default for RenderDocPlugin {
    fn default() -> Self {
        Self {
            key_code: Some(KeyCode::F10),
        }
    }
}

impl RenderDocPlugin {
    /// Create a new RenderDocPlugin with a specific key code set as the trigger key, instead of the default F10.
    pub fn new_with_trigger_key(key_code: KeyCode) -> Self {
        Self {
            key_code: Some(key_code),
        }
    }

    /// Creates a new RenderDocPlugin with no default hook. [RenderDocTrigger] must be used manually to trigger a capture.
    pub fn new_without_trigger() -> Self {
        Self { key_code: None }
    }
}

impl Plugin for RenderDocPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        let renderdoc = match RenderDoc::new() {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Renderdoc could not be loaded, not registering capture hook: {:?}",
                    e
                );
                return;
            }
        };

        info!("Renderdoc available, registering capture hook");

        let sub_app = app.sub_app_mut(RenderApp);

        sub_app.add_systems(ExtractSchedule, start_capture);
        sub_app.add_systems(
            Render,
            after_render_end_capture
                .after(RenderSet::Render)
                .before(RenderSet::Cleanup),
        );

        let is_capture_active = Arc::new(AtomicBool::new(false));

        sub_app.insert_resource(RenderDocData {
            capture_requested: is_capture_active,
            is_capture_active: false,
            api: Mutex::new(renderdoc),
        });

        if let Some(key_code) = self.key_code {
            app.add_systems(
                PostUpdate,
                move |keys: Res<ButtonInput<KeyCode>>, trigger: Res<RenderDocTrigger>| {
                    if keys.just_pressed(key_code) {
                        trigger.capture();
                    }
                },
            );
        }
    }
}

/// Default system for starting a capture, based on a key press.
fn start_capture(mut renderdoc: ResMut<RenderDocData>) {
    if renderdoc
        .capture_requested
        .fetch_and(false, Ordering::SeqCst)
    {
        renderdoc.is_capture_active = true;
        let api = renderdoc.api.get_mut().unwrap();
        api.start_frame_capture(null(), null());
    }
}

/// Post-render disable frame capture if it was on.
fn after_render_end_capture(mut renderdoc: ResMut<RenderDocData>) {
    // Check if the capture is active, and disable it if it is.
    if !renderdoc.is_capture_active {
        return;
    }

    renderdoc.is_capture_active = false;

    let api = renderdoc.api.get_mut().unwrap();
    api.end_frame_capture(null(), null());
}