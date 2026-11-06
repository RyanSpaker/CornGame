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
    // scene file selectors, in `scope`'s subtree: `select_one(e, "DoorState")`, `select(e, ".lamp")`
    select: (scope, selector) => JSON.parse(host.select(scope, selector)),
    select_one: (scope, selector) => JSON.parse(host.select(scope, selector))[0] ?? null,
    // live refs. native: rust objects with property hooks. web: a Proxy over the host's json ref functions
    component: (entity, type) => (host.component ? host.component(entity, type) : live(`c:${entity}`, type)),
    // null if it doesn't exist
    resource: (type) => (host.resource ? host.resource(type) : live("r:", type)),
    // by a Handle field's value (a path, or the id of an asset without one). null if not loaded
    asset: (handle, type) => (host.asset ? host.asset(handle, type) : live(`a:${handle}`, type)),
    trigger: (entity, type, data = {}) => host.trigger(entity ?? null, type, JSON.stringify(data)),
    // another loaded module's exports, by its name or a scene file's path (its module is `path#$script`).
    // they're that module's own objects: changing one changes it for everything calling into it
    module: (name) => modules.get(name) ?? modules.get(`${name}#$script`) ?? null,
    // `yield FixedUpdate`: wait for the next fixed step, and run in it
    FixedUpdate: Object.freeze({ toString: () => "FixedUpdate" }),
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

  // meshes, built in rust as each method is called (corn_scripting's mesh.rs). a value for a
  // `Handle<Mesh>` field, where it becomes an asset (the same one each time, until it changes)
  const meshes = typeof FinalizationRegistry === "function" ? new FinalizationRegistry((id) => host.mesh("drop", id, "", [])) : null;
  // corners as [[x, y, z], ...] or flat
  const corners = (points) => (Array.isArray(points[0]) ? points.flat() : points);
  class Mesh {
    constructor(id) {
      this.id = id;
      meshes?.register(this, id);
    }
    // bevy's Mesh methods. bevy's take the mesh and give it back changed; these change it in place and
    // give it back, so chains read the same. `clone()` for a copy
    clone() {
      return new Mesh(host.mesh("clone", this.id, "", []));
    }
    merge(other) {
      host.mesh("merge", this.id, "", [other.id]);
      return this;
    }
    // a Transform
    transformed_by(t) {
      const { translation: p, rotation: q, scale: s } = t;
      host.mesh("transformed_by", this.id, "", [p.x, p.y, p.z, q.x, q.y, q.z, q.w, s.x, s.y, s.z]);
      return this;
    }
    translated_by(v) {
      return this.transformed_by(types.Transform.from_translation(v));
    }
    rotated_by(q) {
      return this.transformed_by(types.Transform.from_rotation(q));
    }
    scaled_by(v) {
      return this.transformed_by(types.Transform.from_scale(v));
    }
    duplicate_vertices() {
      host.mesh("duplicate_vertices", this.id, "", []);
      return this;
    }
    compute_flat_normals() {
      host.mesh("compute_flat_normals", this.id, "", []);
      return this;
    }
    compute_smooth_normals() {
      host.mesh("compute_smooth_normals", this.id, "", []);
      return this;
    }
    count_vertices() {
      return host.mesh("count_vertices", this.id, "", []);
    }
    // `Mesh.ATTRIBUTE_POSITION` etc; values as [[x, y, z], ...] or flat
    insert_attribute(attribute, values) {
      host.mesh("insert_attribute", this.id, attribute, corners(values));
      return this;
    }
    with_inserted_attribute(attribute, values) {
      return this.insert_attribute(attribute, values);
    }
    insert_indices(indices) {
      host.mesh("insert_indices", this.id, "", indices);
      return this;
    }
    with_inserted_indices(indices) {
      return this.insert_indices(indices);
    }
    toJSON() {
      return `mesh:${this.id}`;
    }
  }
  // without corn_scripting's `mesh` feature the host has no `mesh`, and these throw
  api.Mesh = {
    // a triangle list
    new: () => new Mesh(host.mesh("new", 0, "", [])),
    ATTRIBUTE_POSITION: "POSITION",
    ATTRIBUTE_NORMAL: "NORMAL",
    ATTRIBUTE_UV_0: "UV_0",
    ATTRIBUTE_COLOR: "COLOR",
  };

  // bevy's primitives: `Cuboid.new(1, 2, 1).mesh().build()`, `Plane3d.default().mesh().size(4, 4).build()`,
  // `ConicalFrustum({ radius_top, radius_bottom, height })` (no `new` in bevy). Builders take `resolution`
  const builder = (name, args) => {
    let resolution = null;
    let size = null;
    return {
      resolution(n) {
        resolution = n;
        return this;
      },
      size(width, depth) {
        size = [width, depth];
        return this;
      },
      build() {
        const numbers = size ?? (resolution === null ? args : [...args, resolution]);
        return new Mesh(host.mesh("shape", 0, name, numbers));
      },
    };
  };
  const primitive = (name, args) => ({ mesh: () => builder(name, args) });
  // corn_assets::mesh::SHAPES, with their `new` arguments
  for (const name of ["Cuboid", "Sphere", "Cylinder", "Capsule3d", "Cone", "Torus", "Circle", "Rectangle", "Annulus"]) {
    api[name] = { new: (...args) => primitive(name, args) };
  }
  api.Plane3d = { default: () => primitive("Plane3d", [1, 1]) };
  api.ConicalFrustum = (fields) => primitive("ConicalFrustum", [fields.radius_top, fields.radius_bottom, fields.height]);
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

  // CLAUDE: a value for rust as JSON, for the remote console and `console.log`: never throws. empty for undefined
  const show = (value) => {
    if (value === undefined) return "";
    try {
      const json = JSON.stringify(value, (_, v) =>
        typeof v === "function" ? `[function ${v.name}]` : typeof v === "bigint" ? v.toString() : v,
      );
      if (json !== undefined) return json;
    } catch {}
    return JSON.stringify(String(value));
  };
  const isGenerator = (value) =>
    value !== null && typeof value === "object" && typeof value.next === "function" && typeof value.throw === "function";

  // CLAUDE: native V8's console does nothing: there it goes to rust's log (and the remote console's reply)
  const hostConsole = {};
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    hostConsole[level] = (...args) =>
      host.log(level, args.map((a) => (typeof a === "string" ? a : show(a) || "undefined")).join(" "));
  }

  // CLAUDE: the remote console (corn_scripting's `remote_console` feature, native). screenshots of the primary window, taken in
  // order, one a frame. each starts at once; its generator waits until the file is written, for
  // `yield* screenshot()`. the path is relative to the working directory, or a temp file without one
  const waitShot = function* (id, value) {
    while (!host.shotDone(id)) yield;
    return value;
  };
  function consoleApi() {
    // the absolute path
    api.screenshot = (path) => {
      const [id, absolute] = JSON.parse(host.screenshot(path ?? ""));
      return waitShot(id, absolute);
    };
    // a screenshot each frame for `frames` frames, as `<prefix>_00.png`...; a frame skipped (one still
    // capturing) is a number missing. its generator waits until all were asked for
    api.record = (prefix, frames) => waitShot(host.record(prefix, frames), undefined);
    // quit the app, once screenshots asked for are written
    api.exit = () => host.exit();
  }

  const modules = new Map();
  const threads = new Map();
  // CLAUDE: threads whose final value rust wants (the remote console's)
  const replies = new Set();
  let replGlobals = false;

  return {
    init(h) {
      host = h;
      if (!host.component) webMath();
      types = mathTypes();
      if (host.screenshot) consoleApi();
      if (host.log) globalThis.console = hostConsole;
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
    // seconds to sleep, -2 for FixedUpdate, or -1 when finished
    step(thread, slept) {
      const generator = threads.get(thread);
      if (!generator) return -1;
      let next;
      try {
        next = generator.next(slept);
      } catch (e) {
        threads.delete(thread);
        // the remote console gets the stack
        if (replies.delete(thread)) throw e?.stack ?? e;
        throw e;
      }
      if (next.done) {
        threads.delete(thread);
        if (replies.delete(thread)) host.result(show(next.value));
        return -1;
      }
      if (next.value === api.FixedUpdate) return -2;
      return typeof next.value === "number" ? next.value : 0;
    },
    stop(thread) {
      threads.delete(thread);
      replies.delete(thread);
    },
    // CLAUDE: the remote console. runs `source` as a top-level script, so its declarations stay for the next
    // (`let` can't be declared twice: use `var` or assign to redefine). The host api is global here. A
    // generator value becomes `thread` (true), whose final value goes to `host.result` when it finishes;
    // any other value goes there now
    repl(source, thread) {
      if (!replGlobals) {
        replGlobals = true;
        for (const [name, value] of Object.entries({ ...api, ...types })) if (!(name in globalThis)) globalThis[name] = value;
      }
      source = `${source}\n//# sourceURL=dev`;
      let value;
      try {
        value = host.runScript ? host.runScript(source) : (0, eval)(source);
      } catch (e) {
        throw e?.stack ?? e;
      }
      if (isGenerator(value)) {
        threads.set(thread, value);
        replies.add(thread);
        return true;
      }
      host.result(show(value));
      return false;
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
