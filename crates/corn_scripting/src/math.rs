//! CLAUDE (claude-opus-5-5): the math types scripts use natively: `Vec3`, `Quat`, `Transform`,
//! `GlobalTransform`, with bevy's names (`t.looking_at(target, Vec3.Y)`). This file is the whole api;
//! each engine binds it generically (fields, methods and constructors by name), and values never go
//! through JSON.
//!
//! A value is a [`Slot`]: owned by the script, or a place in a component, reached through a live ref.
//! So `component(e, "Transform").translation.x += 1` writes the component, and `get(e, "Transform")`
//! is a copy. Children share their parent's slot: `const p = t.translation` follows `t`.
//! Methods never change their receiver unless bevy's do (`look_at`, `rotate_around`).

use std::{any::TypeId, cell::RefCell, rc::Rc};

use bevy::prelude::*;

use super::live::{self, LiveRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Vec3,
    Quat,
    Transform,
    GlobalTransform,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Vec3, Kind::Quat, Kind::Transform, Kind::GlobalTransform];

    pub fn of(type_id: TypeId) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.type_id() == type_id)
    }

    fn type_id(self) -> TypeId {
        match self {
            Kind::Vec3 => TypeId::of::<Vec3>(),
            Kind::Quat => TypeId::of::<Quat>(),
            Kind::Transform => TypeId::of::<Transform>(),
            Kind::GlobalTransform => TypeId::of::<GlobalTransform>(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Vec3 => "Vec3",
            Kind::Quat => "Quat",
            Kind::Transform => "Transform",
            Kind::GlobalTransform => "GlobalTransform",
        }
    }
}

/// a math value, copied out of wherever it lives
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Math {
    Vec3(Vec3),
    Quat(Quat),
    Transform(Transform),
    GlobalTransform(GlobalTransform),
}

impl Math {
    pub fn of(value: &dyn PartialReflect) -> Option<Math> {
        let value = value.try_as_reflect()?;
        let value = value.as_any();
        None.or_else(|| value.downcast_ref().copied().map(Math::Vec3))
            .or_else(|| value.downcast_ref().copied().map(Math::Quat))
            .or_else(|| value.downcast_ref().copied().map(Math::Transform))
            .or_else(|| value.downcast_ref().copied().map(Math::GlobalTransform))
    }

    pub fn kind(&self) -> Kind {
        match self {
            Math::Vec3(_) => Kind::Vec3,
            Math::Quat(_) => Kind::Quat,
            Math::Transform(_) => Kind::Transform,
            Math::GlobalTransform(_) => Kind::GlobalTransform,
        }
    }

    pub fn as_reflect(&self) -> &dyn PartialReflect {
        match self {
            Math::Vec3(v) => v,
            Math::Quat(q) => q,
            Math::Transform(t) => t,
            Math::GlobalTransform(g) => g,
        }
    }

    fn as_reflect_mut(&mut self) -> &mut dyn PartialReflect {
        match self {
            Math::Vec3(v) => v,
            Math::Quat(q) => q,
            Math::Transform(t) => t,
            Math::GlobalTransform(g) => g,
        }
    }

    /// for math nested in data (`spawn({ Transform: t })`), which does go through JSON
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Math::Vec3(v) => serde_json::json!(v.to_array()),
            Math::Quat(q) => serde_json::json!(q.to_array()),
            Math::Transform(t) => serde_json::json!({
                "translation": t.translation.to_array(),
                "rotation": t.rotation.to_array(),
                "scale": t.scale.to_array(),
            }),
            Math::GlobalTransform(g) => serde_json::json!(g.affine().to_cols_array()),
        }
    }
}

// CLAUDE
/// where a math value lives
#[derive(Debug, Clone)]
pub enum Slot {
    /// the script's own value, and the path to this part of it
    Owned(Rc<RefCell<Math>>, String),
    Live(LiveRef),
}

impl Slot {
    pub fn new(value: Math) -> Slot {
        Slot::Owned(Rc::new(RefCell::new(value)), String::new())
    }

    fn child(&self, key: &str) -> Slot {
        match self {
            Slot::Owned(root, path) if path.is_empty() => Slot::Owned(root.clone(), key.to_string()),
            Slot::Owned(root, path) => Slot::Owned(root.clone(), format!("{path}.{key}")),
            Slot::Live(r) => Slot::Live(r.child(key)),
        }
    }

    fn get<T: Reflect + TypePath + Copy>(&self) -> Result<T, String> {
        match self {
            Slot::Owned(..) => self.update(|v: &mut T| *v),
            Slot::Live(r) => live::with_value(r, |value| value.try_downcast_ref::<T>().copied())?.ok_or_else(wrong::<T>),
        }
    }

    /// run `f` on the value in place
    fn update<T: Reflect + TypePath, R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, String> {
        match self {
            Slot::Owned(root, path) => {
                let mut root = root.borrow_mut();
                let value = live::walk(root.as_reflect_mut(), path).ok_or_else(|| format!("no field `{path}`"))?;
                value.try_downcast_mut::<T>().map(f).ok_or_else(wrong::<T>)
            }
            Slot::Live(r) => live::with_value_mut(r, |value| value.try_downcast_mut::<T>().map(f))?.ok_or_else(wrong::<T>),
        }
    }

    pub fn value(&self, kind: Kind) -> Result<Math, String> {
        match kind {
            Kind::Vec3 => self.get().map(Math::Vec3),
            Kind::Quat => self.get().map(Math::Quat),
            Kind::Transform => self.get().map(Math::Transform),
            Kind::GlobalTransform => self.get().map(Math::GlobalTransform),
        }
    }
}

fn wrong<T: TypePath>() -> String {
    format!("expected a {}", T::short_type_path())
}

/// a script value going into a method or field
#[derive(Debug, Clone, Copy)]
pub enum Arg {
    Num(f64),
    Math(Math),
}

/// what a field, method or constructor gives the script
#[derive(Debug)]
pub enum Out {
    None,
    Num(f64),
    /// a new value
    Math(Math),
    /// part of an existing one
    Slot(Kind, Slot),
}

// CLAUDE
/// (type, name).
/// static ones are constructors and constants: `Vec3.Y`, `Transform.from_xyz(1, 2, 3)`. names in
/// capitals take no arguments, and js reads them as properties.
pub const STATICS: &[(Kind, &str)] = &[
    (Kind::Vec3, "new"),
    (Kind::Vec3, "splat"),
    (Kind::Vec3, "ZERO"),
    (Kind::Vec3, "ONE"),
    (Kind::Vec3, "X"),
    (Kind::Vec3, "Y"),
    (Kind::Vec3, "Z"),
    (Kind::Vec3, "NEG_Z"),
    (Kind::Quat, "IDENTITY"),
    (Kind::Quat, "from_xyzw"),
    (Kind::Quat, "from_rotation_x"),
    (Kind::Quat, "from_rotation_y"),
    (Kind::Quat, "from_rotation_z"),
    (Kind::Quat, "from_axis_angle"),
    (Kind::Transform, "IDENTITY"),
    (Kind::Transform, "from_xyz"),
    (Kind::Transform, "from_translation"),
    (Kind::Transform, "from_rotation"),
    (Kind::Transform, "from_scale"),
    (Kind::GlobalTransform, "from"),
];

pub const METHODS: &[(Kind, &str)] = &[
    (Kind::Vec3, "add"),
    (Kind::Vec3, "sub"),
    (Kind::Vec3, "mul"),
    (Kind::Vec3, "div"),
    (Kind::Vec3, "neg"),
    (Kind::Vec3, "length"),
    (Kind::Vec3, "normalize"),
    (Kind::Vec3, "normalize_or_zero"),
    (Kind::Vec3, "distance"),
    (Kind::Vec3, "dot"),
    (Kind::Vec3, "cross"),
    (Kind::Vec3, "lerp"),
    (Kind::Vec3, "clone"),
    (Kind::Quat, "mul"),
    (Kind::Quat, "inverse"),
    (Kind::Quat, "normalize"),
    (Kind::Quat, "slerp"),
    (Kind::Quat, "angle_between"),
    (Kind::Quat, "clone"),
    (Kind::Transform, "looking_at"),
    (Kind::Transform, "looking_to"),
    (Kind::Transform, "look_at"),
    (Kind::Transform, "look_to"),
    (Kind::Transform, "rotate"),
    (Kind::Transform, "rotate_around"),
    (Kind::Transform, "with_translation"),
    (Kind::Transform, "with_rotation"),
    (Kind::Transform, "with_scale"),
    (Kind::Transform, "forward"),
    (Kind::Transform, "right"),
    (Kind::Transform, "up"),
    (Kind::Transform, "transform_point"),
    (Kind::Transform, "mul_transform"),
    (Kind::Transform, "clone"),
    (Kind::GlobalTransform, "translation"),
    (Kind::GlobalTransform, "rotation"),
    (Kind::GlobalTransform, "compute_transform"),
    (Kind::GlobalTransform, "forward"),
    (Kind::GlobalTransform, "transform_point"),
    (Kind::GlobalTransform, "clone"),
];

pub fn is_method(kind: Kind, name: &str) -> bool {
    METHODS.iter().any(|(k, n)| *k == kind && *n == name)
}

pub fn statics(kind: Kind) -> impl Iterator<Item = &'static str> {
    STATICS.iter().filter(move |(k, ..)| *k == kind).map(|(_, name)| *name)
}

fn num(args: &[Arg], i: usize) -> Result<f32, String> {
    match args.get(i) {
        Some(Arg::Num(n)) => Ok(*n as f32),
        other => Err(format!("argument {} should be a number, not {other:?}", i + 1)),
    }
}

fn vec3(args: &[Arg], i: usize) -> Result<Vec3, String> {
    match args.get(i) {
        Some(Arg::Math(Math::Vec3(v))) => Ok(*v),
        other => Err(format!("argument {} should be a Vec3, not {other:?}", i + 1)),
    }
}

fn quat(args: &[Arg], i: usize) -> Result<Quat, String> {
    match args.get(i) {
        Some(Arg::Math(Math::Quat(q))) => Ok(*q),
        other => Err(format!("argument {} should be a Quat, not {other:?}", i + 1)),
    }
}

fn transform(args: &[Arg], i: usize) -> Result<Transform, String> {
    match args.get(i) {
        Some(Arg::Math(Math::Transform(t))) => Ok(*t),
        other => Err(format!("argument {} should be a Transform, not {other:?}", i + 1)),
    }
}

// CLAUDE
/// `Kind.name(args)`
pub fn call_static(kind: Kind, name: &str, args: &[Arg]) -> Result<Math, String> {
    let a = args;
    Ok(match (kind, name) {
        (Kind::Vec3, "new") => Math::Vec3(Vec3::new(num(a, 0)?, num(a, 1)?, num(a, 2)?)),
        (Kind::Vec3, "splat") => Math::Vec3(Vec3::splat(num(a, 0)?)),
        (Kind::Vec3, "ZERO") => Math::Vec3(Vec3::ZERO),
        (Kind::Vec3, "ONE") => Math::Vec3(Vec3::ONE),
        (Kind::Vec3, "X") => Math::Vec3(Vec3::X),
        (Kind::Vec3, "Y") => Math::Vec3(Vec3::Y),
        (Kind::Vec3, "Z") => Math::Vec3(Vec3::Z),
        (Kind::Vec3, "NEG_Z") => Math::Vec3(Vec3::NEG_Z),
        (Kind::Quat, "IDENTITY") => Math::Quat(Quat::IDENTITY),
        (Kind::Quat, "from_xyzw") => Math::Quat(Quat::from_xyzw(num(a, 0)?, num(a, 1)?, num(a, 2)?, num(a, 3)?)),
        (Kind::Quat, "from_rotation_x") => Math::Quat(Quat::from_rotation_x(num(a, 0)?)),
        (Kind::Quat, "from_rotation_y") => Math::Quat(Quat::from_rotation_y(num(a, 0)?)),
        (Kind::Quat, "from_rotation_z") => Math::Quat(Quat::from_rotation_z(num(a, 0)?)),
        (Kind::Quat, "from_axis_angle") => Math::Quat(Quat::from_axis_angle(vec3(a, 0)?, num(a, 1)?)),
        (Kind::Transform, "IDENTITY") => Math::Transform(Transform::IDENTITY),
        (Kind::Transform, "from_xyz") => Math::Transform(Transform::from_xyz(num(a, 0)?, num(a, 1)?, num(a, 2)?)),
        (Kind::Transform, "from_translation") => Math::Transform(Transform::from_translation(vec3(a, 0)?)),
        (Kind::Transform, "from_rotation") => Math::Transform(Transform::from_rotation(quat(a, 0)?)),
        (Kind::Transform, "from_scale") => Math::Transform(Transform::from_scale(vec3(a, 0)?)),
        (Kind::GlobalTransform, "from") => Math::GlobalTransform(transform(a, 0)?.into()),
        _ => return Err(format!("{} has no `{name}`", kind.name())),
    })
}

// CLAUDE
/// a field: numbers for vectors and quaternions, parts of the slot for transforms. None if there's no
/// such field
pub fn get_field(kind: Kind, slot: &Slot, name: &str) -> Result<Option<Out>, String> {
    let component = |v: &[f32], i: usize| Out::Num(v[i] as f64);
    Ok(Some(match (kind, name) {
        (Kind::Vec3, "x" | "y" | "z") => component(&slot.get::<Vec3>()?.to_array(), xyzw(name)),
        (Kind::Quat, "x" | "y" | "z" | "w") => component(&slot.get::<Quat>()?.to_array(), xyzw(name)),
        (Kind::Transform, "translation" | "scale") => Out::Slot(Kind::Vec3, slot.child(name)),
        (Kind::Transform, "rotation") => Out::Slot(Kind::Quat, slot.child(name)),
        _ => return Ok(None),
    }))
}

fn xyzw(name: &str) -> usize {
    match name {
        "y" => 1,
        "z" => 2,
        "w" => 3,
        _ => 0,
    }
}

/// set a field. false if there's no such field
pub fn set_field(kind: Kind, slot: &Slot, name: &str, value: Arg) -> Result<bool, String> {
    let value = [value];
    match (kind, name) {
        (Kind::Vec3, "x" | "y" | "z") => {
            let n = num(&value, 0)?;
            slot.update(|v: &mut Vec3| v[xyzw(name)] = n)?;
        }
        (Kind::Quat, "x" | "y" | "z" | "w") => {
            let n = num(&value, 0)?;
            slot.update(|q: &mut Quat| {
                let mut a = q.to_array();
                a[xyzw(name)] = n;
                *q = Quat::from_array(a);
            })?;
        }
        (Kind::Transform, "translation" | "scale") => {
            let new = vec3(&value, 0)?;
            slot.child(name).update(|v: &mut Vec3| *v = new)?;
        }
        (Kind::Transform, "rotation") => {
            let new = quat(&value, 0)?;
            slot.child(name).update(|q: &mut Quat| *q = new)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

// CLAUDE
/// `value.name(args)`
pub fn call(kind: Kind, slot: &Slot, name: &str, args: &[Arg]) -> Result<Out, String> {
    let a = args;
    let math = |m| Ok(Out::Math(m));
    let number = |n: f32| Ok(Out::Num(n as f64));
    match (slot.value(kind)?, name) {
        (Math::Vec3(v), "add") => math(Math::Vec3(v + vec3(a, 0)?)),
        (Math::Vec3(v), "sub") => math(Math::Vec3(v - vec3(a, 0)?)),
        (Math::Vec3(v), "mul") => match a.first() {
            Some(Arg::Num(n)) => math(Math::Vec3(v * *n as f32)),
            _ => math(Math::Vec3(v * vec3(a, 0)?)),
        },
        (Math::Vec3(v), "div") => math(Math::Vec3(v / num(a, 0)?)),
        (Math::Vec3(v), "neg") => math(Math::Vec3(-v)),
        (Math::Vec3(v), "length") => number(v.length()),
        (Math::Vec3(v), "normalize") => math(Math::Vec3(v.normalize())),
        (Math::Vec3(v), "normalize_or_zero") => math(Math::Vec3(v.normalize_or_zero())),
        (Math::Vec3(v), "distance") => number(v.distance(vec3(a, 0)?)),
        (Math::Vec3(v), "dot") => number(v.dot(vec3(a, 0)?)),
        (Math::Vec3(v), "cross") => math(Math::Vec3(v.cross(vec3(a, 0)?))),
        (Math::Vec3(v), "lerp") => math(Math::Vec3(v.lerp(vec3(a, 0)?, num(a, 1)?))),
        (Math::Quat(q), "mul") => match a.first() {
            Some(Arg::Math(Math::Vec3(v))) => math(Math::Vec3(q * *v)),
            _ => math(Math::Quat(q * quat(a, 0)?)),
        },
        (Math::Quat(q), "inverse") => math(Math::Quat(q.inverse())),
        (Math::Quat(q), "normalize") => math(Math::Quat(q.normalize())),
        (Math::Quat(q), "slerp") => math(Math::Quat(q.slerp(quat(a, 0)?, num(a, 1)?))),
        (Math::Quat(q), "angle_between") => number(q.angle_between(quat(a, 0)?)),
        (Math::Transform(t), "looking_at") => math(Math::Transform(t.looking_at(vec3(a, 0)?, vec3(a, 1)?))),
        (Math::Transform(t), "looking_to") => math(Math::Transform(t.looking_to(vec3(a, 0)?, vec3(a, 1)?))),
        (Math::Transform(_), "look_at") => {
            let (target, up) = (vec3(a, 0)?, vec3(a, 1)?);
            slot.update(|t: &mut Transform| t.look_at(target, up)).map(|()| Out::None)
        }
        (Math::Transform(_), "look_to") => {
            let (direction, up) = (vec3(a, 0)?, vec3(a, 1)?);
            slot.update(|t: &mut Transform| t.look_to(direction, up)).map(|()| Out::None)
        }
        (Math::Transform(_), "rotate") => {
            let rotation = quat(a, 0)?;
            slot.update(|t: &mut Transform| t.rotate(rotation)).map(|()| Out::None)
        }
        (Math::Transform(_), "rotate_around") => {
            let (point, rotation) = (vec3(a, 0)?, quat(a, 1)?);
            slot.update(|t: &mut Transform| t.rotate_around(point, rotation)).map(|()| Out::None)
        }
        (Math::Transform(t), "with_translation") => math(Math::Transform(t.with_translation(vec3(a, 0)?))),
        (Math::Transform(t), "with_rotation") => math(Math::Transform(t.with_rotation(quat(a, 0)?))),
        (Math::Transform(t), "with_scale") => math(Math::Transform(t.with_scale(vec3(a, 0)?))),
        (Math::Transform(t), "forward") => math(Math::Vec3(*t.forward())),
        (Math::Transform(t), "right") => math(Math::Vec3(*t.right())),
        (Math::Transform(t), "up") => math(Math::Vec3(*t.up())),
        (Math::Transform(t), "transform_point") => math(Math::Vec3(t.transform_point(vec3(a, 0)?))),
        (Math::Transform(t), "mul_transform") => math(Math::Transform(t.mul_transform(transform(a, 0)?))),
        (Math::GlobalTransform(g), "translation") => math(Math::Vec3(g.translation())),
        (Math::GlobalTransform(g), "rotation") => math(Math::Quat(g.rotation())),
        (Math::GlobalTransform(g), "compute_transform") => math(Math::Transform(g.compute_transform())),
        (Math::GlobalTransform(g), "forward") => math(Math::Vec3(*g.forward())),
        (Math::GlobalTransform(g), "transform_point") => math(Math::Vec3(g.transform_point(vec3(a, 0)?))),
        (value, "clone") => math(value),
        _ => Err(format!("{} has no method `{name}`", kind.name())),
    }
}
