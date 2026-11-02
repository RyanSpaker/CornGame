//! CLAUDE (claude-sonnet-5-5): native js engine (V8)
//!
//! Live refs and math values are objects with named property interceptors, each wrapping its [`Handle`]
//! as a cppgc (V8's C++ heap) object, so it lives exactly as long as the js object.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    rc::{self, Rc},
    sync::{Once, OnceLock},
};

use super::{
    host_despawn, host_find, host_get, host_get_resource, host_insert, host_insert_math, host_remove, host_spawn,
    host_trigger,
    live::{self, Field, LiveRef, Read},
    math::{self, Arg, Kind, Math, Out, Slot},
    Engine, Got, GLUE,
};

type Scope<'s, 'i> = v8::PinScope<'s, 'i>;

pub struct Js {
    // field order: everything holding V8 handles drops before the isolate
    context: v8::Global<v8::Context>,
    glue: v8::Global<v8::Object>,
    // kept alive here: the isolate slot only holds a weak reference
    _shared: Rc<Shared>,
    isolate: RefCell<v8::OwnedIsolate>,
}

/// what a ref object stands for
enum Handle {
    Live(LiveRef),
    Math(Kind, Slot),
}

// SAFETY: holds no traced references. dropped on the engine's thread: the heap sweeps incrementally, not
// concurrently (see `Js::new`)
unsafe impl v8::cppgc::GarbageCollected for Handle {
    fn trace(&self, _: &mut v8::cppgc::Visitor) {}

    fn get_name(&self) -> &'static std::ffi::CStr {
        c"ScriptRef"
    }
}

/// the wrapper tag of [`Handle`]s
const HANDLE: u16 = 1;

/// the engine's state that callbacks reach, through an isolate slot
struct Shared {
    component_template: v8::Global<v8::ObjectTemplate>,
    math_template: v8::Global<v8::ObjectTemplate>,
}

fn shared(scope: &Scope<'_, '_>) -> Rc<Shared> {
    scope.get_slot::<rc::Weak<Shared>>().and_then(rc::Weak::upgrade).expect("the js engine is gone")
}

thread_local! {
    /// the scope js is running in, while it runs. nested calls (a host call that runs a command that runs a script) reuse it
    static ACTIVE: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
}

/// marks `scope` active until dropped
struct Active(*mut c_void);

impl Active {
    fn set(scope: &mut Scope<'_, '_>) -> Self {
        Active(ACTIVE.replace(scope as *mut _ as *mut c_void))
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.set(self.0);
    }
}

fn platform() -> v8::SharedRef<v8::Platform> {
    static PLATFORM: OnceLock<v8::SharedRef<v8::Platform>> = OnceLock::new();
    static INIT: Once = Once::new();
    let platform = PLATFORM.get_or_init(|| v8::new_default_platform(0, false).make_shared()).clone();
    INIT.call_once(|| {
        v8::V8::initialize_platform(platform.clone());
        v8::cppgc::initialize_process(platform.clone());
        v8::V8::initialize();
    });
    platform
}

fn string<'s>(scope: &Scope<'s, '_>, s: &str) -> v8::Local<'s, v8::String> {
    v8::String::new(scope, s).unwrap()
}

fn throw(scope: &mut Scope<'_, '_>, message: impl AsRef<str>) {
    let message = string(scope, message.as_ref());
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
}

fn text(scope: &mut Scope<'_, '_>, value: v8::Local<v8::Value>) -> String {
    value.to_rust_string_lossy(scope)
}

impl Js {
    pub fn new() -> Result<Self, String> {
        // sweeping on this thread only: dropping a `Handle` touches `Rc`s
        let heap = v8::cppgc::Heap::create(
            platform(),
            v8::cppgc::HeapCreateParams { sweeping_support: v8::cppgc::SweepingType::Incremental, ..Default::default() },
        );
        let mut isolate = v8::Isolate::new(v8::CreateParams::default().cpp_heap(heap));
        let (context, glue, shared) = {
            v8::scope!(let scope, &mut isolate);
            let context = v8::Context::new(scope, Default::default());
            let mut context_scope = v8::ContextScope::new(scope, context);
            v8::scope!(let scope, &mut context_scope);
            let template = |scope: &mut Scope<'_, '_>, config: v8::NamedPropertyHandlerConfiguration| {
                let t = v8::ObjectTemplate::new(scope);
                t.set_named_property_handler(config);
                v8::Global::new(scope, t)
            };
            let shared = Rc::new(Shared {
                component_template: {
                    let t = template(scope, v8::NamedPropertyHandlerConfiguration::new().getter(component_get).setter(component_set));
                    let local = v8::Local::new(scope, &t);
                    local.set_indexed_property_handler(v8::IndexedPropertyHandlerConfiguration::new().getter(component_index_get).setter(component_index_set));
                    t
                },
                math_template: {
                    let t = template(scope, v8::NamedPropertyHandlerConfiguration::new().getter(math_get).setter(math_set));
                    // a real property too: JSON.stringify doesn't look for `toJSON` through the interceptor
                    let local = v8::Local::new(scope, &t);
                    let name = string(scope, "toJSON");
                    let to_json = v8::FunctionTemplate::builder(math_method).data(name.into()).build(scope);
                    local.set(name.into(), to_json.into());
                    t
                },
            });
            scope.set_slot(Rc::downgrade(&shared));
            let _active = Active::set(scope);
            let glue = Self::init(scope)?;
            (v8::Global::new(scope, context), v8::Global::new(scope, glue), shared)
        };
        Ok(Self { context, glue, _shared: shared, isolate: RefCell::new(isolate) })
    }

    fn init<'s>(scope: &mut Scope<'s, '_>) -> Result<v8::Local<'s, v8::Object>, String> {
        v8::tc_scope!(let tc, scope);
        let error = |tc: &mut v8::PinnedRef<v8::TryCatch<v8::HandleScope>>| {
            tc.exception().map(|e| e.to_rust_string_lossy(tc)).unwrap_or_else(|| "unknown js error".into())
        };
        let source = string(tc, GLUE);
        let glue = v8::Script::compile(tc, source, None).and_then(|s| s.run(tc)).ok_or_else(|| error(tc))?;
        let glue = glue.cast::<v8::Object>();

        let host = v8::Object::new(tc);
        macro_rules! host_fns {
            ($($name:literal => $f:ident),* $(,)?) => {$(
                let key = string(tc, $name);
                let f = v8::Function::new(tc, $f).unwrap();
                host.set(tc, key.into(), f.into());
            )*};
        }
        host_fns! {
            "insert" => insert, "get" => get, "getResource" => get_resource, "remove" => remove, "spawn" => spawn,
            "despawn" => despawn, "find" => find, "component" => component, "resource" => resource, "asset" => asset,
            "trigger" => trigger, "result" => result, "useApp" => use_app,
            "isMath" => is_math, "mathStatics" => math_statics, "mathStatic" => math_static,
        }
        let init = string(tc, "init");
        let init = glue.get(tc, init.into()).unwrap().cast::<v8::Function>();
        init.call(tc, glue.into(), &[host.into()]).ok_or_else(|| error(tc))?;
        Ok(glue)
    }

    /// call a glue method, entering the context, or reusing the active scope if js is already running
    fn invoke(&self, name: &str, args: impl for<'s, 'i> FnOnce(&mut Scope<'s, 'i>) -> Vec<v8::Local<'s, v8::Value>>) -> Result<f64, String> {
        fn run<'s, 'i>(
            scope: &mut Scope<'s, 'i>,
            glue: &v8::Global<v8::Object>,
            name: &str,
            args: impl FnOnce(&mut Scope<'s, 'i>) -> Vec<v8::Local<'s, v8::Value>>,
        ) -> Result<f64, String> {
            let args = args(scope);
            v8::tc_scope!(let tc, scope);
            let glue = v8::Local::new(tc, glue);
            let key = string(tc, name);
            let f = glue.get(tc, key.into()).unwrap().cast::<v8::Function>();
            match f.call(tc, glue.into(), &args) {
                Some(v) if v.is_boolean() => Ok(v.boolean_value(tc) as u8 as f64),
                Some(v) => Ok(v.number_value(tc).unwrap_or(0.0)),
                None => Err(tc.exception().map(|e| e.to_rust_string_lossy(tc)).unwrap_or_else(|| "unknown js error".into())),
            }
        }

        let active = ACTIVE.get();
        if !active.is_null() {
            // SAFETY: ACTIVE is only set while that scope is live, further up this stack. it is this engine's:
            // an app has one, and only its own host calls run scripts
            let scope = unsafe { &mut *(active as *mut Scope<'static, 'static>) };
            return run(scope, &self.glue, name, args);
        }
        let mut isolate = self.isolate.borrow_mut();
        v8::scope!(let scope, &mut *isolate);
        let context = v8::Local::new(scope, &self.context);
        let mut context_scope = v8::ContextScope::new(scope, context);
        v8::scope!(let scope, &mut context_scope);
        let _active = Active::set(scope);
        run(scope, &self.glue, name, args)
    }
}

impl Engine for Js {
    fn load(&self, module: &str, source: &str) -> Result<(), String> {
        self.invoke("load", |s| vec![string(s, module).into(), string(s, source).into()]).map(|_| ())
    }

    fn call(&self, module: &str, function: &str, thread: u32, target: Option<&str>, args: &str, event: &str) -> Result<bool, String> {
        self.invoke("call", |s| {
            vec![
                string(s, module).into(),
                string(s, function).into(),
                v8::Number::new(s, thread as f64).into(),
                target.map_or_else(|| v8::null(s).into(), |t| string(s, t).into()),
                string(s, args).into(),
                string(s, event).into(),
            ]
        })
        .map(|r| r != 0.0)
    }

    fn step(&self, thread: u32, slept: f64) -> Result<Option<f64>, String> {
        let sleep = self.invoke("step", |s| vec![v8::Number::new(s, thread as f64).into(), v8::Number::new(s, slept).into()])?;
        Ok((sleep >= 0.0).then_some(sleep))
    }

    fn stop(&self, thread: u32) {
        let _ = self.invoke("stop", |s| vec![v8::Number::new(s, thread as f64).into()]);
    }

    fn evaluate(&self, module: &str, function: &str, target: &str, root: &str) -> Result<String, String> {
        self.invoke("evaluate", |s| {
            vec![string(s, module).into(), string(s, function).into(), string(s, target).into(), string(s, root).into()]
        })?;
        Ok(super::RESULT.take().unwrap_or_default())
    }
}

// host functions

fn arg_string(scope: &mut Scope<'_, '_>, args: &v8::FunctionCallbackArguments, i: i32) -> String {
    let v = args.get(i);
    text(scope, v)
}

fn insert(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, _: v8::ReturnValue) {
    let _active = Active::set(scope);
    let entity = arg_string(scope, &args, 0);
    let type_name = arg_string(scope, &args, 1);
    let value = args.get(2);
    let result = match math_value(scope, value) {
        Ok(Some(math)) => host_insert_math(&entity, &type_name, math),
        Ok(None) => host_insert(&entity, &type_name, &text(scope, value)),
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        throw(scope, e);
    }
}

fn got_js<'s>(scope: &mut Scope<'s, '_>, got: Got) -> v8::Local<'s, v8::Value> {
    match got {
        Got::Json(json) => string(scope, &json).into(),
        Got::Math(math) => new_math(scope, math.kind(), Slot::new(math)),
    }
}

fn get(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let entity = arg_string(scope, &args, 0);
    let type_name = arg_string(scope, &args, 1);
    match host_get(&entity, &type_name) {
        Ok(got) => rv.set(got_js(scope, got)),
        Err(e) => throw(scope, e),
    }
}

fn get_resource(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let type_name = arg_string(scope, &args, 0);
    match host_get_resource(&type_name) {
        Ok(got) => rv.set(got_js(scope, got)),
        Err(e) => throw(scope, e),
    }
}

fn remove(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, _: v8::ReturnValue) {
    let _active = Active::set(scope);
    let (entity, type_name) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    if let Err(e) = host_remove(&entity, &type_name) {
        throw(scope, e);
    }
}

fn spawn(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let json = arg_string(scope, &args, 0);
    match host_spawn(&json) {
        Ok(e) => rv.set(string(scope, &e).into()),
        Err(e) => throw(scope, e),
    }
}

fn despawn(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, _: v8::ReturnValue) {
    let _active = Active::set(scope);
    let entity = arg_string(scope, &args, 0);
    if let Err(e) = host_despawn(&entity) {
        throw(scope, e);
    }
}

fn result(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, _: v8::ReturnValue) {
    let json = arg_string(scope, &args, 0);
    super::host_result(json);
}

fn use_app(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, _: v8::ReturnValue) {
    let on = args.get(0).boolean_value(scope);
    if let Err(e) = super::host_use_app(on) {
        throw(scope, e);
    }
}

fn find(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let type_name = arg_string(scope, &args, 0);
    match host_find(&type_name) {
        Ok(json) => rv.set(string(scope, &json).into()),
        Err(e) => throw(scope, e),
    }
}

fn trigger(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, _: v8::ReturnValue) {
    let _active = Active::set(scope);
    let target = (!args.get(0).is_null_or_undefined()).then(|| arg_string(scope, &args, 0));
    let (event, json) = (arg_string(scope, &args, 1), arg_string(scope, &args, 2));
    if let Err(e) = host_trigger(target.as_deref(), &event, &json) {
        throw(scope, e);
    }
}

fn component(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let (entity, type_name) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    let r = live::component(&entity, &type_name).map(Some);
    return_ref(scope, r, rv);
}

fn resource(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let type_name = arg_string(scope, &args, 0);
    let r = live::resource(&type_name);
    return_ref(scope, r, rv);
}

fn asset(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let (key, type_name) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    let r = live::asset(&key, &type_name);
    return_ref(scope, r, rv);
}

/// a root live ref as its object (a math value for math types), or null
fn return_ref(scope: &mut Scope<'_, '_>, r: Result<Option<LiveRef>, String>, mut rv: v8::ReturnValue) {
    match r {
        Ok(Some(r)) => match Kind::of(r.root_type()) {
            Some(kind) => rv.set(new_math(scope, kind, Slot::Live(r))),
            None => rv.set(new_object(scope, Handle::Live(r), false)),
        },
        Ok(None) => rv.set_null(),
        Err(e) => throw(scope, e),
    }
}

fn is_math(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let is = with_handle(scope, args.get(0), |h| matches!(h, Handle::Math(..))).unwrap_or(false);
    rv.set_bool(is);
}

fn math_statics(scope: &mut Scope<'_, '_>, _: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let types = v8::Object::new(scope);
    for kind in Kind::ALL {
        let names: Vec<v8::Local<v8::Value>> = math::statics(kind).map(|n| string(scope, n).into()).collect();
        let names = v8::Array::new_with_elements(scope, &names);
        let key = string(scope, kind.name());
        types.set(scope, key.into(), names.into());
    }
    rv.set(types.into());
}

fn math_static(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let kind_name = arg_string(scope, &args, 0);
    let name = arg_string(scope, &args, 1);
    let Some(kind) = Kind::ALL.into_iter().find(|k| k.name() == kind_name) else {
        return throw(scope, format!("no math type `{kind_name}`"));
    };
    let list = args.get(2).cast::<v8::Array>();
    let mut call_args = Vec::new();
    for i in 0..list.length() {
        let v = list.get_index(scope, i).unwrap();
        match to_arg(scope, v) {
            Ok(a) => call_args.push(a),
            Err(e) => return throw(scope, e),
        }
    }
    match math::call_static(kind, &name, &call_args) {
        Ok(math) => rv.set(new_math(scope, kind, Slot::new(math))),
        Err(e) => throw(scope, e),
    }
}

// objects

fn new_object<'s>(scope: &mut Scope<'s, '_>, handle: Handle, is_math: bool) -> v8::Local<'s, v8::Value> {
    let shared = shared(scope);
    let template = v8::Local::new(scope, if is_math { &shared.math_template } else { &shared.component_template });
    let object = template.new_instance(scope).unwrap();
    // SAFETY: the pointer goes straight into the wrapper
    let handle = unsafe { v8::cppgc::make_garbage_collected(scope.get_cpp_heap().unwrap(), handle) };
    // SAFETY: a fresh object from our template
    unsafe { v8::Object::wrap::<HANDLE, Handle>(scope, object, &handle) };
    object.into()
}

fn new_math<'s>(scope: &mut Scope<'s, '_>, kind: Kind, slot: Slot) -> v8::Local<'s, v8::Value> {
    new_object(scope, Handle::Math(kind, slot), true)
}

/// `f` of the handle a ref object wraps, None if `value` isn't one
fn with_handle<R>(scope: &mut Scope<'_, '_>, value: v8::Local<v8::Value>, f: impl FnOnce(&Handle) -> R) -> Option<R> {
    let object = v8::Local::<v8::Object>::try_from(value).ok().filter(|o| o.is_api_wrapper())?;
    // SAFETY: the object is alive (it's in a local), so is the handle it wraps
    let handle = unsafe { v8::Object::unwrap::<HANDLE, Handle>(scope, object)? };
    Some(f(unsafe { handle.as_ref() }))
}

fn math_value(scope: &mut Scope<'_, '_>, value: v8::Local<v8::Value>) -> Result<Option<Math>, String> {
    match with_handle(scope, value, math_of).flatten() {
        Some((kind, slot)) => slot.value(kind).map(Some),
        None => Ok(None),
    }
}

fn to_arg(scope: &mut Scope<'_, '_>, value: v8::Local<v8::Value>) -> Result<Arg, String> {
    if value.is_number() {
        return Ok(Arg::Num(value.number_value(scope).unwrap()));
    }
    match math_value(scope, value)? {
        Some(math) => Ok(Arg::Math(math)),
        None => Err("expected a number or a math value".into()),
    }
}

fn from_out<'s>(scope: &mut Scope<'s, '_>, out: Out) -> v8::Local<'s, v8::Value> {
    match out {
        Out::None => v8::undefined(scope).into(),
        Out::Num(n) => v8::Number::new(scope, n).into(),
        Out::Math(math) => new_math(scope, math.kind(), Slot::new(math)),
        Out::Slot(kind, slot) => new_math(scope, kind, slot),
    }
}

fn to_js<'s>(scope: &mut Scope<'s, '_>, field: Field) -> v8::Local<'s, v8::Value> {
    match field {
        Field::Null => v8::null(scope).into(),
        Field::Bool(b) => v8::Boolean::new(scope, b).into(),
        Field::Int(i) => v8::Number::new(scope, i as f64).into(),
        Field::Float(f) => v8::Number::new(scope, f).into(),
        Field::Str(s) => string(scope, &s).into(),
        Field::Json(json) => {
            let s = string(scope, &json.to_string());
            v8::json::parse(scope, s).unwrap_or_else(|| v8::undefined(scope).into())
        }
        Field::Math(math) => new_math(scope, math.kind(), Slot::new(math)),
    }
}

fn from_js(scope: &mut Scope<'_, '_>, value: v8::Local<v8::Value>) -> Result<Field, String> {
    if value.is_null_or_undefined() {
        return Ok(Field::Null);
    }
    if let Some(math) = math_value(scope, value)? {
        return Ok(Field::Math(math));
    }
    if value.is_boolean() {
        return Ok(Field::Bool(value.boolean_value(scope)));
    }
    if value.is_int32() {
        return Ok(Field::Int(value.int32_value(scope).unwrap().into()));
    }
    if value.is_number() {
        return Ok(Field::Float(value.number_value(scope).unwrap()));
    }
    if value.is_string() {
        return Ok(Field::Str(text(scope, value)));
    }
    let json = v8::json::stringify(scope, value).map(|s| s.to_rust_string_lossy(scope)).unwrap_or_default();
    serde_json::from_str(&json).map(Field::Json).map_err(|e| e.to_string())
}

/// the handle of the object an interceptor was called on, cloned out
fn this_handle(scope: &mut Scope<'_, '_>, args: &v8::PropertyCallbackArguments) -> Result<HandleCopy, String> {
    let copy = |h: &Handle| match h {
        Handle::Live(r) => HandleCopy::Live(r.clone()),
        Handle::Math(kind, slot) => HandleCopy::Math(*kind, slot.clone()),
    };
    with_handle(scope, args.holder().into(), copy).ok_or_else(|| "not a script reference".into())
}

fn math_of(h: &Handle) -> Option<(Kind, Slot)> {
    match h {
        Handle::Math(kind, slot) => Some((*kind, slot.clone())),
        Handle::Live(_) => None,
    }
}

enum HandleCopy {
    Live(LiveRef),
    Math(Kind, Slot),
}

type KeyBuffer = [std::mem::MaybeUninit<u8>; 64];

/// a property name, in `buffer` if it fits. None for symbols
fn key_name<'b>(scope: &mut Scope<'_, '_>, key: v8::Local<v8::Name>, buffer: &'b mut KeyBuffer) -> Option<std::borrow::Cow<'b, str>> {
    let key: v8::Local<v8::Value> = key.into();
    (!key.is_symbol()).then(|| key.cast::<v8::String>().to_rust_cow_lossy(scope, buffer))
}

fn component_get(scope: &mut Scope<'_, '_>, key: v8::Local<v8::Name>, args: v8::PropertyCallbackArguments, rv: v8::ReturnValue) -> v8::Intercepted {
    let mut buffer: KeyBuffer = [std::mem::MaybeUninit::uninit(); 64];
    match key_name(scope, key, &mut buffer) {
        Some(key) => component_read(scope, &key, args, rv),
        None => v8::Intercepted::kNo,
    }
}

/// `ref[0]`: numeric keys come to an indexed handler, not the named one
fn component_index_get(scope: &mut Scope<'_, '_>, index: u32, args: v8::PropertyCallbackArguments, rv: v8::ReturnValue) -> v8::Intercepted {
    component_read(scope, &index.to_string(), args, rv)
}

fn component_read(scope: &mut Scope<'_, '_>, key: &str, args: v8::PropertyCallbackArguments, mut rv: v8::ReturnValue) -> v8::Intercepted {
    let _active = Active::set(scope);
    let this = match this_handle(scope, &args) {
        Ok(HandleCopy::Live(r)) => r,
        Ok(_) => return v8::Intercepted::kNo,
        Err(e) => {
            throw(scope, e);
            return v8::Intercepted::kYes;
        }
    };
    let child = this.child(key);
    match live::read(&child) {
        Ok(Read::Value(field)) => rv.set(to_js(scope, field)),
        Ok(Read::Ref(r)) => rv.set(new_object(scope, Handle::Live(r), false)),
        Ok(Read::Math(kind, r)) => rv.set(new_math(scope, kind, Slot::Live(r))),
        Ok(Read::Missing) => rv.set(v8::undefined(scope).into()),
        Err(e) => throw(scope, e),
    }
    v8::Intercepted::kYes
}

fn component_set(scope: &mut Scope<'_, '_>, key: v8::Local<v8::Name>, value: v8::Local<v8::Value>, args: v8::PropertyCallbackArguments, rv: v8::ReturnValue<v8::Boolean>) -> v8::Intercepted {
    let mut buffer: KeyBuffer = [std::mem::MaybeUninit::uninit(); 64];
    match key_name(scope, key, &mut buffer) {
        Some(key) => component_write(scope, &key, value, args, rv),
        None => v8::Intercepted::kNo,
    }
}

fn component_index_set(scope: &mut Scope<'_, '_>, index: u32, value: v8::Local<v8::Value>, args: v8::PropertyCallbackArguments, rv: v8::ReturnValue<v8::Boolean>) -> v8::Intercepted {
    component_write(scope, &index.to_string(), value, args, rv)
}

fn component_write(scope: &mut Scope<'_, '_>, key: &str, value: v8::Local<v8::Value>, args: v8::PropertyCallbackArguments, mut rv: v8::ReturnValue<v8::Boolean>) -> v8::Intercepted {
    let _active = Active::set(scope);
    let result = (|| {
        let this = match this_handle(scope, &args)? {
            HandleCopy::Live(r) => r,
            _ => return Err("not a live reference".to_string()),
        };
        live::write(&this.child(key), from_js(scope, value)?)
    })();
    match result {
        Ok(()) => rv.set_bool(true),
        Err(e) => throw(scope, e),
    }
    v8::Intercepted::kYes
}

fn math_get(scope: &mut Scope<'_, '_>, key: v8::Local<v8::Name>, args: v8::PropertyCallbackArguments, mut rv: v8::ReturnValue) -> v8::Intercepted {
    let _active = Active::set(scope);
    let mut buffer: KeyBuffer = [std::mem::MaybeUninit::uninit(); 64];
    let Some(key) = key_name(scope, key, &mut buffer) else {
        return v8::Intercepted::kNo;
    };
    let (kind, slot) = match this_handle(scope, &args) {
        Ok(HandleCopy::Math(kind, slot)) => (kind, slot),
        Ok(_) => return v8::Intercepted::kNo,
        Err(e) => {
            throw(scope, e);
            return v8::Intercepted::kYes;
        }
    };
    if key == "toJSON" || math::is_method(kind, &key) {
        let name = string(scope, &key);
        let f = v8::Function::builder(math_method).data(name.into()).build(scope).unwrap();
        rv.set(f.into());
        return v8::Intercepted::kYes;
    }
    match math::get_field(kind, &slot, &key) {
        Ok(Some(out)) => rv.set(from_out(scope, out)),
        Ok(None) => rv.set(v8::undefined(scope).into()),
        Err(e) => throw(scope, e),
    }
    v8::Intercepted::kYes
}

fn math_set(scope: &mut Scope<'_, '_>, key: v8::Local<v8::Name>, value: v8::Local<v8::Value>, args: v8::PropertyCallbackArguments, mut rv: v8::ReturnValue<v8::Boolean>) -> v8::Intercepted {
    let _active = Active::set(scope);
    let mut buffer: KeyBuffer = [std::mem::MaybeUninit::uninit(); 64];
    let Some(key) = key_name(scope, key, &mut buffer) else {
        return v8::Intercepted::kNo;
    };
    let result = (|| {
        let (kind, slot) = match this_handle(scope, &args)? {
            HandleCopy::Math(kind, slot) => (kind, slot),
            _ => return Err("not a math reference".to_string()),
        };
        let value = to_arg(scope, value)?;
        math::set_field(kind, &slot, &key, value)
    })();
    match result {
        Ok(ok) => rv.set_bool(ok),
        Err(e) => throw(scope, e),
    }
    v8::Intercepted::kYes
}

/// a math method (or `toJSON`), called on a math object. the function's data is the method name
fn math_method(scope: &mut Scope<'_, '_>, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let _active = Active::set(scope);
    let name = text(scope, args.data());
    let this: v8::Local<v8::Value> = args.this().into();
    let result: Result<v8::Local<v8::Value>, String> = (|| {
        let (kind, slot) = with_handle(scope, this, math_of).flatten().ok_or("not a math value")?;
        if name == "toJSON" {
            let json = slot.value(kind)?.to_json().to_string();
            let s = string(scope, &json);
            return Ok(v8::json::parse(scope, s).unwrap_or_else(|| v8::undefined(scope).into()));
        }
        let mut call_args = Vec::new();
        for i in 0..args.length() {
            call_args.push(to_arg(scope, args.get(i))?);
        }
        let out = math::call(kind, &slot, &name, &call_args)?;
        Ok(from_out(scope, out))
    })();
    match result {
        Ok(v) => rv.set(v),
        Err(e) => throw(scope, e),
    }
}

