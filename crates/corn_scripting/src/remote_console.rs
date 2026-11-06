//! CODEX (GPT-6): debug builds default to port 7777; CORN_DEV overrides the port.
//! CLAUDE (claude-opus-5-5): the remote console, for agents (and people) poking at a running app. native
//! only, with the `remote_console` feature.
//!
//! **Scripts** get `screenshot(path?)`, `record(prefix, frames)` and `exit()` (see glue.js `consoleApi`).
//! Screenshots are of the primary window, taken in the order asked for, one a frame.
//!
//! **The socket.** Debug builds listen on 127.0.0.1:7777 by default. `CORN_DEV=<port>` overrides
//! the port (empty: 7777) and enables the socket in release builds too. Each line sent is
//! a JSON request, answered by one JSON line:
//! - `{"code": "..."}` runs the code as a top-level script (glue.js `repl`): its declarations stay for
//!   later requests, and the host api is global. The reply is its value. If that value is a generator
//!   (`screenshot()` gives one, so does calling a `function*`), it runs over frames like any script
//!   thread and the reply waits for its final value.
//! - `{"code": "...", "generator": true}` runs the code as a generator's body: it may `yield` (seconds,
//!   or a generator with `yield*`), and the reply is what it `return`s.
//! - the reply: `{"ok": true, "value": ..., "logs": [...]}` (no `value` for undefined) or
//!   `{"ok": false, "error": "...", "logs": [...]}`. `logs` is the request's `console.log` output.
//!
//! Requests run once a frame, in [`ScriptSystems`] after the script driver. `crates/corn_dev` is a
//! command line client.

use std::{
    collections::{HashSet, VecDeque},
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
};

use bevy::{
    prelude::*,
    render::view::screenshot::{save_to_disk, Capturing, Screenshot, ScreenshotCaptured},
};

use super::{as_module, capture_logs, drive_scripts, with_host_world, with_world_scope, Engine, ScriptEngine, ScriptSystems, Thread, Threads, Waiter, RESULT};

const DEFAULT_PORT: u16 = 7777;

pub fn plugin(app: &mut App) {
    app.init_resource::<Shots>()
        .add_systems(PostUpdate, (serve.in_set(ScriptSystems).after(drive_scripts), take_shots.after(ScriptSystems)));
    // CODEX: enable local inspection by default in debug builds, preserving explicit release opt-in.
    if let Some(port) = std::env::var("CORN_DEV").ok()
        .or_else(|| cfg!(debug_assertions).then(|| DEFAULT_PORT.to_string())) {
        let port = if port.trim().is_empty() { DEFAULT_PORT } else { port.trim().parse().expect("CORN_DEV is a port") };
        let (send, receive) = mpsc::channel();
        app.insert_resource(Requests(Mutex::new(receive)));
        std::thread::Builder::new().name("remote console".into()).spawn(move || listen(port, send)).unwrap();
    }
}

// the socket

struct Request {
    code: String,
    generator: bool,
    reply: mpsc::Sender<String>,
}

#[derive(Resource)]
struct Requests(Mutex<mpsc::Receiver<Request>>);

fn listen(port: u16, requests: mpsc::Sender<Request>) {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(e) => return error!("remote console: can't listen on 127.0.0.1:{port}: {e}"),
    };
    info!("remote console on 127.0.0.1:{port}");
    for stream in listener.incoming().flatten() {
        let requests = requests.clone();
        let _ = std::thread::Builder::new().name("remote console connection".into()).spawn(move || connection(stream, requests));
    }
}

/// answers a connection's requests in turn
fn connection(stream: TcpStream, requests: mpsc::Sender<Request>) {
    let Ok(mut out) = stream.try_clone() else { return };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(request) => {
                let code = request["code"].as_str().unwrap_or_default().to_string();
                let generator = request["generator"].as_bool().unwrap_or(false);
                let (reply, answer) = mpsc::channel();
                if requests.send(Request { code, generator, reply }).is_err() {
                    return;
                }
                // dropped unanswered: the app stopped its thread (or is closing)
                answer.recv().unwrap_or_else(|_| reply_json(Err("stopped before it finished".into()), Vec::new()))
            }
            Err(e) => reply_json(Err(format!("not a JSON request: {e}")), Vec::new()),
        };
        if writeln!(out, "{reply}").is_err() {
            return;
        }
    }
}

fn reply_json(result: Result<String, String>, logs: Vec<String>) -> String {
    let mut reply = serde_json::json!({ "ok": result.is_ok(), "logs": logs });
    match result {
        Ok(json) if json.is_empty() => {}
        Ok(json) => reply["value"] = serde_json::from_str(&json).unwrap_or(serde_json::Value::String(json)),
        Err(e) => reply["error"] = e.into(),
    }
    reply.to_string()
}

/// runs the requests that came in since last frame
fn serve(world: &mut World) {
    let Some(requests) = world.get_resource::<Requests>() else { return };
    let requests: Vec<Request> = requests.0.lock().unwrap().try_iter().collect();
    if requests.is_empty() {
        return;
    }
    let engine = world.non_send::<ScriptEngine>().0.clone();
    for request in requests {
        run(world, &engine, request);
    }
}

/// runs a request to its end, or its first yield (then it's a thread the driver steps, which answers)
fn run(world: &mut World, engine: &super::js::Js, request: Request) {
    let id = {
        let mut threads = world.resource_mut::<Threads>();
        threads.next = threads.next.wrapping_add(1);
        threads.next
    };
    let source = match request.generator {
        true => format!("(function* () {{\n{}\n}})()", request.code),
        false => request.code,
    };
    // the value is in RESULT when finished
    let (result, logs) = capture_logs(Some(Vec::new()), || {
        with_world_scope(world, || {
            as_module("dev", || match engine.repl(&source, id)? {
                true => engine.step(id, 0.0),
                false => Ok(None),
            })
        })
    });
    let logs = logs.unwrap_or_default();
    let reply = request.reply;
    match result {
        Ok(None) => drop(reply.send(reply_json(Ok(RESULT.take().unwrap_or_default()), logs))),
        Ok(Some(wait)) => {
            let done = Box::new(move |result, logs| drop(reply.send(reply_json(result, logs))));
            let thread = Thread {
                id,
                owner: None,
                name: "dev".into(),
                module: "dev".into(),
                wait,
                slept: 0.0,
                waiter: Some(Waiter { logs, done }),
            };
            world.resource_mut::<Threads>().running.push(thread);
        }
        Err(e) => drop(reply.send(reply_json(Err(e), logs))),
    }
}

// screenshots

#[derive(Resource, Default)]
struct Shots {
    next: u32,
    /// taken in order
    queue: VecDeque<Job>,
    /// screenshots not written yet, recordings not done
    pending: HashSet<u32>,
    /// `exit()`: quit once the queue is done and written
    exit: bool,
}

enum Job {
    Shot { id: u32, path: PathBuf },
    Record { id: u32, prefix: PathBuf, done: u32, frames: u32 },
}

/// relative to the working directory, its folder made
fn absolute(path: impl AsRef<Path>) -> Result<PathBuf, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Ok(path)
}

fn with_shots<R>(f: impl FnOnce(&mut Shots) -> Result<R, String>) -> Result<R, String> {
    with_host_world(|world| f(&mut *world.get_resource_mut::<Shots>().ok_or("no screenshots while a scene builds")?))
}

/// `[id, absolute path]` as JSON
pub fn host_screenshot(path: &str) -> Result<String, String> {
    with_shots(|shots| {
        shots.next += 1;
        let id = shots.next;
        let mut path = match path {
            "" => std::env::temp_dir().join("corn_dev").join(format!("shot_{id}.png")),
            path => PathBuf::from(path),
        };
        if path.extension().is_none() {
            path.set_extension("png");
        }
        let path = absolute(path)?;
        let json = serde_json::json!([id, path.display().to_string()]).to_string();
        shots.queue.push_back(Job::Shot { id, path });
        shots.pending.insert(id);
        Ok(json)
    })
}

pub fn host_record(prefix: &str, frames: u32) -> Result<u32, String> {
    let prefix = absolute(prefix)?;
    with_shots(|shots| {
        shots.next += 1;
        let id = shots.next;
        if frames > 0 {
            shots.queue.push_back(Job::Record { id, prefix, done: 0, frames });
            shots.pending.insert(id);
        }
        Ok(id)
    })
}

pub fn host_shot_done(id: u32) -> Result<bool, String> {
    with_shots(|shots| Ok(!shots.pending.contains(&id)))
}

pub fn host_exit() -> Result<(), String> {
    with_shots(|shots| {
        shots.exit = true;
        Ok(())
    })
}

/// works through the queue, one screenshot a frame: bevy takes one of a window a frame, and drops a
/// second asked for before the first starts
fn take_shots(
    mut commands: Commands,
    mut shots: ResMut<Shots>,
    starting: Query<(), (With<Screenshot>, Without<Capturing>)>,
    screenshots: Query<(), With<Screenshot>>,
    mut exit: MessageWriter<AppExit>,
) {
    let free = starting.is_empty();
    let mut shoot = |id: Option<u32>, path: PathBuf| {
        let mut screenshot = commands.spawn(Screenshot::primary_window());
        screenshot.observe(save_to_disk(path));
        if let Some(id) = id {
            screenshot.observe(move |_: On<ScreenshotCaptured>, mut shots: ResMut<Shots>| {
                shots.pending.remove(&id);
            });
        }
    };
    let shots = &mut *shots;
    match shots.queue.front_mut() {
        None if shots.exit && screenshots.is_empty() => {
            exit.write(AppExit::Success);
        }
        None => {}
        Some(Job::Shot { id, path }) => {
            if free {
                shoot(Some(*id), std::mem::take(path));
                shots.queue.pop_front();
            }
        }
        // a frame skipped (the last still starting) is a number missing
        Some(Job::Record { id, prefix, done, frames }) => {
            if free {
                let mut path = prefix.clone().into_os_string();
                path.push(format!("_{done:02}.png"));
                shoot(None, path.into());
            }
            *done += 1;
            if done >= frames {
                shots.pending.remove(id);
                shots.queue.pop_front();
            }
        }
    }
}

// CLAUDE
#[cfg(test)]
mod tests {
    use super::*;

    /// an app with the remote console, and a way to send it requests
    fn app() -> (App, mpsc::Sender<Request>) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default())).add_plugins(crate::plugin);
        let (send, receive) = mpsc::channel();
        app.insert_resource(Requests(Mutex::new(receive)));
        (app, send)
    }

    fn ask(requests: &mpsc::Sender<Request>, code: &str, generator: bool) -> mpsc::Receiver<String> {
        let (reply, answer) = mpsc::channel();
        requests.send(Request { code: code.into(), generator, reply }).unwrap();
        answer
    }

    fn answer(app: &mut App, answer: &mpsc::Receiver<String>) -> serde_json::Value {
        app.update();
        serde_json::from_str(&answer.try_recv().expect("answered this frame")).unwrap()
    }

    #[test]
    fn declarations_stay_values_and_logs_come_back() {
        let (mut app, requests) = app();
        let reply = |app: &mut App, code| answer(app, &ask(&requests, code, false));
        assert_eq!(reply(&mut app, "var x = 2; let y = 1; x * 21")["value"], 42);
        assert_eq!(reply(&mut app, "x + y")["value"], 3);
        let logged = reply(&mut app, r#"console.log("hi", { a: 1 }); console.warn("careful"); undefined"#);
        assert_eq!(logged, serde_json::json!({ "ok": true, "logs": ["hi {\"a\":1}", "[warn] careful"] }));
        // the host api is global
        let e = reply(&mut app, "var e = spawn({}); e")["value"].as_str().unwrap().to_string();
        let entity = Entity::from_bits(e.parse().unwrap());
        assert!(app.world().get_entity(entity).is_ok());
        assert_eq!(reply(&mut app, "despawn(e); Vec3.new(1, 2, 3)")["value"], serde_json::json!([1, 2, 3]));
        assert!(app.world().get_entity(entity).is_err());
        let failed = reply(&mut app, "nope()");
        assert_eq!(failed["ok"], false);
        let error = failed["error"].as_str().unwrap();
        assert!(error.contains("ReferenceError") && error.contains("at dev:1"), "{error}");
    }

    #[test]
    fn generators_answer_when_they_finish() {
        let (mut app, requests) = app();
        let body = ask(&requests, "console.log('first'); yield; console.log('second'); return 7", true);
        app.update();
        assert!(body.try_recv().is_err(), "still running");
        assert_eq!(answer(&mut app, &body), serde_json::json!({ "ok": true, "value": 7, "logs": ["first", "second"] }));
        // a generator value runs too, and its errors come back
        let value = ask(&requests, "function* g() { yield; throw new Error('late'); } g()", false);
        app.update();
        let failed = answer(&mut app, &value);
        assert!(failed["error"].as_str().unwrap().contains("late"), "{failed}");
    }
}
