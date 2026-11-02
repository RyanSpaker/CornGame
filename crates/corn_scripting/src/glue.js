// CLAUDE (claude-opus-5-5): js half of the script runtime, shared by V8 (native) and the browser (web).
// Evaluates to an object. Rust calls init once, then load/call/step/stop. Modules are keyed by name,
// generators by thread id. Calls may nest: a host call can run a command that calls back in here.
(() => {
  let host;
  // the globals a module sees. values cross to rust as JSON, except math values (Vec3, Transform...)
  const api = {
    insert: (entity, type, value) =>
      isMath(value)
        ? (host.insertMath ?? host.insert)(entity, type, unwrap(value))
        : host.insert(entity, type, JSON.stringify(value ?? {})),
    get: (entity, type) => copy(host.get(entity, type)),
    get_resource: (type) => copy(host.getResource(type)),
    remove: (entity, type) => host.remove(entity, type),
    spawn: (components = {}) => host.spawn(JSON.stringify(components)),
    despawn: (entity) => host.despawn(entity),
    find: (type) => JSON.parse(host.find(type)),
    // live refs. native: rust objects with property hooks. web: a Proxy over the host's json ref functions
    component: (entity, type) => (host.component ? host.component(entity, type) : live(`c:${entity}`, type)),
    // null if it doesn't exist
    resource: (type) => (host.resource ? host.resource(type) : live("r:", type)),
    // by a Handle field's value (a path, or the id of an asset without one). null if not loaded
    asset: (handle, type) => (host.asset ? host.asset(handle, type) : live(`a:${handle}`, type)),
    trigger: (entity, type, data = {}) => host.trigger(entity ?? null, type, JSON.stringify(data)),
  };
  // while a scene file builds, the api above acts on the scene and these read the app's world
  const inApp =
    (f) =>
    (...args) => {
      host.useApp(true);
      try {
        return f(...args);
      } finally {
        host.useApp(false);
      }
    };
  api.world = { get: inApp(api.get), get_resource: inApp(api.get_resource), find: inApp(api.find) };

  // meshes from shapes, built in rust (corn_assets::mesh). a value for a `Handle<Mesh>` field
  class Mesh {
    constructor(ops = []) {
      this.ops = ops;
    }
    // another mesh, moved: rotate is degrees (XYZ), scale a number or [x, y, z]
    add(part, { translate = [0, 0, 0], rotate = [0, 0, 0], scale = [1, 1, 1] } = {}) {
      scale = typeof scale === "number" ? [scale, scale, scale] : scale;
      this.ops.push({ Part: { ops: part.ops, translate, rotate, scale } });
      return this;
    }
    // a grid on XZ: size [x, z], cells [x, z], heights row by row ((cells + 1) each way), or none for flat
    heightfield(size, cells, heights = []) {
      this.ops.push({ Heightfield: { size, cells, heights: Array.from(heights) } });
      return this;
    }
    noise({ seed = 0, frequency = 1, amplitude = 1, octaves = 1 } = {}) {
      this.ops.push({ Noise: { seed, frequency, amplitude, octaves } });
      return this;
    }
    smooth_normals() {
      this.ops.push("SmoothNormals");
      return this;
    }
    flat_normals() {
      this.ops.push("FlatNormals");
      return this;
    }
    toJSON() {
      return { ops: this.ops };
    }
  }
  api.mesh = () => new Mesh();
  for (const name of ["Cuboid", "Sphere", "Cylinder", "Capsule3d", "Cone", "Torus", "Plane3d"]) {
    api[name] = (...args) => new Mesh([{ Shape: { name, args } }]);
  }
  // Vec3, Quat, Transform, GlobalTransform: their constructors and constants
  let types = {};

  // math values. native: rust objects. web: numeric handles, wrapped in a Proxy here
  let wrap = (value) => value;
  let unwrap = (value) => value;
  let isMath = (value) => host.isMath(value);
  let arg = (value) => value;
  const out = (value) => (value !== null && typeof value === "object" ? wrap(value) : value);
  // what get gives: JSON, or a math value
  const copy = (value) => (typeof value === "string" ? JSON.parse(value) : out(value));

  function webMath() {
    const HANDLE = Symbol("math");
    const methods = host.mathMethods();
    const dropped = new FinalizationRegistry((h) => host.mathDrop(h));
    wrap = ({ h, kind }) => {
      const proxy = new Proxy(
        { h },
        {
          get(target, key) {
            if (key === HANDLE) return target;
            if (typeof key !== "string") return undefined;
            if (key === "toJSON") return () => JSON.parse(host.mathJson(h));
            if (methods[kind].includes(key)) return (...args) => out(host.mathCall(h, key, args.map(arg)));
            return out(host.mathGet(h, key));
          },
          set(_, key, value) {
            host.mathSet(h, key, arg(value));
            return true;
          },
        },
      );
      dropped.register(proxy, h);
      return proxy;
    };
    unwrap = (value) => value[HANDLE].h;
    isMath = (value) => typeof value === "object" && value !== null && value[HANDLE] !== undefined;
    arg = (value) => (isMath(value) ? { h: unwrap(value) } : value);
  }

  function mathTypes() {
    const types = {};
    for (const [kind, names] of Object.entries(host.mathStatics())) {
      const type = (types[kind] = {});
      for (const name of names) {
        const make = (...args) => out(host.mathStatic(kind, name, args.map(arg)));
        // constants are properties: Vec3.Y
        if (name === name.toUpperCase()) Object.defineProperty(type, name, { get: make, enumerable: true });
        else type[name] = make;
      }
    }
    return types;
  }

  // base: `c:entity` (component), `r:` (resource) or `a:handle` (asset)
  function live(base, type, path = "") {
    const at = (key) => (path ? `${path}.${key}` : key);
    if (!path) {
      // throws if the entity lacks the component
      const read = JSON.parse(host.refGet(base, type, ""));
      if (!read.ref && !read.math) return null;
      if (read.math) return wrap(host.refMath(base, type, ""));
    }
    return new Proxy(
      {},
      {
        get(_, key) {
          if (typeof key !== "string") return undefined;
          const read = JSON.parse(host.refGet(base, type, at(key)));
          if (read.math) return wrap(host.refMath(base, type, at(key)));
          return read.ref ? live(base, type, at(key)) : read.value;
        },
        set(_, key, value) {
          if (typeof key !== "string") return false;
          if (isMath(value)) host.refSetMath(base, type, at(key), unwrap(value));
          else host.refSet(base, type, at(key), JSON.stringify(value));
          return true;
        },
      },
    );
  }

  const modules = new Map();
  const threads = new Map();

  return {
    init(h) {
      host = h;
      if (!host.component) webMath();
      types = mathTypes();
    },
    // evaluate a module once; its top-level variables are its state
    load(name, source) {
      const module = { exports: {} };
      const globals = { ...api, ...types };
      new Function("module", "exports", ...Object.keys(globals), `${source}\n//# sourceURL=${name}`)(
        module,
        module.exports,
        ...Object.values(globals),
      );
      modules.set(name, module.exports);
    },
    // true if the function returned a generator
    call(name, fn, thread, target, args, event) {
      const f = modules.get(name)?.[fn];
      if (typeof f !== "function") throw new Error(`${name} exports no function \`${fn}\``);
      const result = f(target, JSON.parse(args), JSON.parse(event));
      if (!result || typeof result.next !== "function") return false;
      threads.set(thread, result);
      return true;
    },
    // seconds to sleep, or -1 when finished
    step(thread, slept) {
      const generator = threads.get(thread);
      if (!generator) return -1;
      let next;
      try {
        next = generator.next(slept);
      } catch (e) {
        threads.delete(thread);
        throw e;
      }
      if (next.done) {
        threads.delete(thread);
        return -1;
      }
      return typeof next.value === "number" ? next.value : 0;
    },
    stop(thread) {
      threads.delete(thread);
    },
    // a function's value, handed to rust as JSON (empty for undefined)
    evaluate(name, fn, target, root) {
      const f = modules.get(name)?.[fn];
      if (typeof f !== "function") throw new Error(`${name} exports no function \`${fn}\``);
      const value = f(target, root);
      if (value && typeof value.next === "function") throw new Error("a value can't yield");
      host.result(value === undefined ? "" : JSON.stringify(value));
      return 0;
    },
  };
})()
