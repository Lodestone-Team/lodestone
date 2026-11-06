// Lodestone op compatibility shim. Loaded as an extension entry point after the
// Deno runtime extensions, before bootstrap. See `register_compat_shim` in mod.rs.
// Keep this file ASCII-only: it is loaded as an external one-byte V8 string.
import * as allOps from "ext:core/ops";
import { internals } from "ext:core/mod.js";

const NAMES = __LODESTONE_OP_NAMES__;

// `internals.core` is the frozen, user-visible copy of `core` that
// `Deno[Deno.internal].core` resolves to.
const prev = internals.core;
if (prev == null) {
  throw new Error("lodestone compat: internals.core is not set");
}

const ops = { __proto__: null };
for (let i = 0; i < NAMES.length; i++) {
  const name = NAMES[i];
  // Without a startup snapshot, `ext:core/ops` holds every op but `prev.ops` has
  // already been pruned. With a snapshot, `ext:core/ops` is restored from the
  // snapshot (no Lodestone ops) but `prev.ops` still has them until bootstrap.
  const f = allOps[name] ?? prev.ops[name];
  if (typeof f !== "function") {
    throw new Error("lodestone compat: op not bound: " + name);
  }
  ops[name] = f;
}

// Pre-deno_core-0.2xx API: `core.opAsync(name, ...args)` returning a Promise.
// Async ops already return promises; this only forwards the call.
function opAsync(name, ...args) {
  const f = ops[name];
  if (typeof f !== "function") {
    return Promise.reject(new TypeError("Unknown op: " + name));
  }
  try {
    return Promise.resolve(f(...args));
  } catch (e) {
    return Promise.reject(e);
  }
}

function opSync(name, ...args) {
  const f = ops[name];
  if (typeof f !== "function") {
    throw new TypeError("Unknown op: " + name);
  }
  return f(...args);
}

const descriptors = Object.getOwnPropertyDescriptors(prev);
descriptors.ops = { value: ops, enumerable: true };
descriptors.opAsync = { value: opAsync, enumerable: true };
descriptors.opSync = { value: opSync, enumerable: true };
internals.core = Object.freeze(
  Object.defineProperties({ __proto__: null }, descriptors),
);
