//! CLAUDE (claude-opus-5-5): web js engine. scripts run in the browser's own engine, which allows nested
//! calls (js -> wasm host call -> js) as is; the host closures are `Fn`, so they may be re-entered too.

use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::{prelude::*, JsCast};

use std::{cell::RefCell, collections::HashMap};

use super::{
    host_despawn, host_find, host_get, host_get_resource, host_insert, host_insert_math, host_remove, host_spawn,
    host_trigger, live,
    math::{self, Arg, Kind, Out, Slot},
    Engine, Got, GLUE,
};

pub struct Js {
    glue: JsValue,
    load: Function,
    call: Function,
    step: Function,
    stop: Function,
    evaluate: Function,
}

impl Js {
    pub fn new() -> Result<Self, String> {
        let glue = js_sys::eval(GLUE).map_err(describe)?;
        let method = |name: &str| {
            Reflect::get(&glue, &name.into())
                .and_then(JsCast::dyn_into::<Function>)
                .map_err(describe)
        };

        // into_js_value hands the closures to the js gc, which keeps them alive
        let host = Object::new();
        let set = |name: &str, f: JsValue| Reflect::set(&host, &name.into(), &f).map_err(describe);
        let undefined = |()| JsValue::UNDEFINED;
        set("insert", closure3(move |e, t, json| host_insert(&e, &t, &json).map(undefined)))?;
        set("get", closure2(|e, t| host_get(&e, &t).map(got_js)))?;
        set("getResource", closure1(|t| host_get_resource(&t).map(got_js)))?;
        set("insertMath", host_fn(move |e, t, h, _| host_insert_math(&string(&e)?, &string(&t)?, math_of(&h)?).map(undefined)))?;
        set("refMath", host_fn(|base, t, path, _| {
            let r = live::json::at(&string(&base)?, &string(&t)?, &string(&path)?)?;
            let kind = match path.as_string().as_deref() {
                Some("") => Kind::of(r.root_type()),
                _ => match live::read(&r)? {
                    live::Read::Math(kind, _) => Some(kind),
                    _ => None,
                },
            };
            Ok(handle(kind.ok_or("not a math value")?, Slot::Live(r)))
        }))?;
        set("refSetMath", host_fn(move |base, t, path, h| {
            let r = live::json::at(&string(&base)?, &string(&t)?, &string(&path)?)?;
            live::write(&r, live::Field::Math(math_of(&h)?)).map(undefined)
        }))?;
        set("mathStatics", host_fn(|_, _, _, _| Ok(names(|kind| math::statics(kind).collect()))))?;
        set("mathMethods", host_fn(|_, _, _, _| {
            Ok(names(|kind| math::METHODS.iter().filter(|(k, ..)| *k == kind).map(|(_, n)| *n).collect()))
        }))?;
        set("mathStatic", host_fn(|kind, name, args, _| {
            let kind = Kind::ALL.into_iter().find(|k| Some(k.name()) == kind.as_string().as_deref()).ok_or("no such math type")?;
            let math = math::call_static(kind, &string(&name)?, &args_of(&args)?)?;
            Ok(handle(kind, Slot::new(math)))
        }))?;
        set("mathGet", host_fn(|h, key, _, _| {
            let (kind, slot) = slot_of(&h)?;
            math::get_field(kind, &slot, &string(&key)?)?.map_or(Ok(JsValue::UNDEFINED), from_out)
        }))?;
        set("mathSet", host_fn(|h, key, value, _| {
            let (kind, slot) = slot_of(&h)?;
            math::set_field(kind, &slot, &string(&key)?, arg_of(&value)?).map(|_| JsValue::UNDEFINED)
        }))?;
        set("mathCall", host_fn(|h, name, args, _| {
            let (kind, slot) = slot_of(&h)?;
            from_out(math::call(kind, &slot, &string(&name)?, &args_of(&args)?)?)
        }))?;
        set("mathJson", host_fn(|h, _, _, _| {
            let (kind, slot) = slot_of(&h)?;
            Ok(JsValue::from(slot.value(kind)?.to_json().to_string()))
        }))?;
        set("mathDrop", host_fn(|h, _, _, _| {
            HANDLES.with_borrow_mut(|handles| handles.1.remove(&(h.as_f64().unwrap_or(-1.0) as u32)));
            Ok(JsValue::UNDEFINED)
        }))?;
        set("remove", closure2(move |e, t| host_remove(&e, &t).map(undefined)))?;
        set("spawn", closure1(|json| host_spawn(&json).map(JsValue::from)))?;
        set("despawn", closure1(move |e| host_despawn(&e).map(undefined)))?;
        set("find", closure1(|t| host_find(&t).map(JsValue::from)))?;
        set("result", closure1(move |json| {
            super::host_result(json);
            Ok(JsValue::UNDEFINED)
        }))?;
        set("useApp", host_fn(move |on, _, _, _| super::host_use_app(on.is_truthy()).map(undefined)))?;
        set("refGet", closure3(|base, t, path| live::json::get(&base, &t, &path).map(JsValue::from)))?;
        set("refSet", closure4(move |base, t, path, json| live::json::set(&base, &t, &path, &json).map(undefined)))?;
        let trigger = move |target: JsValue, event: String, json: String| {
            host_trigger(target.as_string().as_deref(), &event, &json).map(undefined).map_err(throw)
        };
        set("trigger", Closure::<dyn Fn(JsValue, String, String) -> Result<JsValue, JsValue>>::new(trigger).into_js_value())?;
        method("init")?.call1(&glue, &host).map_err(describe)?;

        let (load, call, step, stop) = (method("load")?, method("call")?, method("step")?, method("stop")?);
        let evaluate = method("evaluate")?;
        Ok(Self { glue, load, call, step, stop, evaluate })
    }
}

impl Engine for Js {
    fn load(&self, module: &str, source: &str) -> Result<(), String> {
        self.load.call2(&self.glue, &module.into(), &source.into()).map(drop).map_err(describe)
    }

    fn call(&self, module: &str, function: &str, thread: u32, target: Option<&str>, args: &str, event: &str) -> Result<bool, String> {
        let target = target.map_or(JsValue::NULL, JsValue::from);
        let params: [JsValue; 6] = [module.into(), function.into(), thread.into(), target, args.into(), event.into()];
        let params = params.into_iter().collect::<Array>();
        Ok(self.call.apply(&self.glue, &params).map_err(describe)?.is_truthy())
    }

    fn step(&self, thread: u32, slept: f64) -> Result<Option<f64>, String> {
        let sleep = self.step.call2(&self.glue, &thread.into(), &slept.into()).map_err(describe)?;
        let sleep = sleep.as_f64().unwrap_or(-1.0);
        Ok((sleep >= 0.0).then_some(sleep))
    }

    fn stop(&self, thread: u32) {
        let _ = self.stop.call1(&self.glue, &thread.into());
    }

    fn evaluate(&self, module: &str, function: &str, target: &str, root: &str) -> Result<String, String> {
        let params: [JsValue; 4] = [module.into(), function.into(), target.into(), root.into()];
        self.evaluate.apply(&self.glue, &params.into_iter().collect::<Array>()).map_err(describe)?;
        Ok(super::RESULT.take().unwrap_or_default())
    }
}

type HostResult = Result<JsValue, String>;

// each js signature gets one `Closure::new` instantiation: wasm-bindgen 0.2.105 exports one symbol per
// instantiation, and generic copies of the same signature collide. so the rust closures are boxed

type Host1 = Box<dyn Fn(String) -> HostResult>;
type Host2 = Box<dyn Fn(String, String) -> HostResult>;
type Host3 = Box<dyn Fn(String, String, String) -> HostResult>;
type Host4 = Box<dyn Fn(String, String, String, String) -> HostResult>;
type HostAny = Box<dyn Fn(JsValue, JsValue, JsValue, JsValue) -> HostResult>;

fn closure1(f: impl Fn(String) -> HostResult + 'static) -> JsValue {
    let f: Host1 = Box::new(f);
    Closure::<dyn Fn(String) -> Result<JsValue, JsValue>>::new(move |a| f(a).map_err(throw)).into_js_value()
}

fn closure2(f: impl Fn(String, String) -> HostResult + 'static) -> JsValue {
    let f: Host2 = Box::new(f);
    Closure::<dyn Fn(String, String) -> Result<JsValue, JsValue>>::new(move |a, b| f(a, b).map_err(throw)).into_js_value()
}

fn closure3(f: impl Fn(String, String, String) -> HostResult + 'static) -> JsValue {
    let f: Host3 = Box::new(f);
    Closure::<dyn Fn(String, String, String) -> Result<JsValue, JsValue>>::new(move |a, b, c| f(a, b, c).map_err(throw))
        .into_js_value()
}

fn closure4(f: impl Fn(String, String, String, String) -> HostResult + 'static) -> JsValue {
    let f: Host4 = Box::new(f);
    Closure::<dyn Fn(String, String, String, String) -> Result<JsValue, JsValue>>::new(move |a, b, c, d| {
        f(a, b, c, d).map_err(throw)
    })
    .into_js_value()
}

fn throw(message: String) -> JsValue {
    js_sys::Error::new(&message).into()
}

fn describe(error: JsValue) -> String {
    let message = error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .unwrap_or_else(|| format!("{error:?}"));
    let stack = Reflect::get(&error, &"stack".into()).ok().and_then(|s| s.as_string());
    format!("{message}\n{}", stack.unwrap_or_default())
}

// CLAUDE
/// a host function of up to 4 arguments, any of them js values (missing ones are undefined)
fn host_fn(f: impl Fn(JsValue, JsValue, JsValue, JsValue) -> HostResult + 'static) -> JsValue {
    let f: HostAny = Box::new(f);
    Closure::<dyn Fn(JsValue, JsValue, JsValue, JsValue) -> Result<JsValue, JsValue>>::new(move |a, b, c, d| {
        f(a, b, c, d).map_err(throw)
    })
    .into_js_value()
}

fn string(value: &JsValue) -> Result<String, String> {
    value.as_string().ok_or_else(|| format!("expected a string, got {value:?}"))
}

thread_local! {
    /// math values the glue holds, by handle. the glue drops them when their js object is collected
    static HANDLES: RefCell<(u32, HashMap<u32, (Kind, Slot)>)> = RefCell::default();
}

/// a math value for the glue: `{ h, kind }`
fn handle(kind: Kind, slot: Slot) -> JsValue {
    let h = HANDLES.with_borrow_mut(|(next, handles)| {
        *next = next.wrapping_add(1);
        handles.insert(*next, (kind, slot));
        *next
    });
    let object = Object::new();
    let _ = Reflect::set(&object, &"h".into(), &h.into());
    let _ = Reflect::set(&object, &"kind".into(), &kind.name().into());
    object.into()
}

/// a handle number
fn slot_of(h: &JsValue) -> Result<(Kind, Slot), String> {
    let h = h.as_f64().ok_or("expected a math handle")? as u32;
    HANDLES.with_borrow(|(_, handles)| handles.get(&h).cloned()).ok_or_else(|| "a dropped math value".into())
}

fn math_of(h: &JsValue) -> Result<math::Math, String> {
    let (kind, slot) = slot_of(h)?;
    slot.value(kind)
}

/// a number, or `{ h }` for a math value
fn arg_of(value: &JsValue) -> Result<Arg, String> {
    if let Some(n) = value.as_f64() {
        return Ok(Arg::Num(n));
    }
    let h = Reflect::get(value, &"h".into()).map_err(describe)?;
    math_of(&h).map(Arg::Math)
}

fn args_of(args: &JsValue) -> Result<Vec<Arg>, String> {
    Array::from(args).iter().map(|a| arg_of(&a)).collect()
}

fn from_out(out: Out) -> HostResult {
    Ok(match out {
        Out::None => JsValue::UNDEFINED,
        Out::Num(n) => JsValue::from(n),
        Out::Math(math) => handle(math.kind(), Slot::new(math)),
        Out::Slot(kind, slot) => handle(kind, slot),
    })
}

/// `{ Vec3: [...], ... }`
fn names(of: impl Fn(Kind) -> Vec<&'static str>) -> JsValue {
    let object = Object::new();
    for kind in Kind::ALL {
        let names: Array = of(kind).into_iter().map(JsValue::from).collect();
        let _ = Reflect::set(&object, &kind.name().into(), &names);
    }
    object.into()
}

/// what `get` gives: a JSON string, or a math handle
fn got_js(got: Got) -> JsValue {
    match got {
        Got::Json(json) => JsValue::from(json),
        Got::Math(math) => handle(math.kind(), Slot::new(math)),
    }
}
