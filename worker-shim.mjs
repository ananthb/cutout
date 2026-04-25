// Worker entrypoint shim.
//
// worker-build wires the wasm-bindgen `email` export onto the entrypoint
// prototype directly: `Entrypoint.prototype.email = V`. But V's signature
// is `(message, env, ctx)` and Cloudflare invokes `entrypoint.email(message)`,
// so env and ctx come through as `undefined`, the runtime context is
// missing, and any subsequent `await` in the handler hangs until the CPU
// budget is exceeded. (See the equivalent fetch wiring in the same file,
// which IS wrapped to pass `this.env, this.ctx`.)
//
// This shim intercepts the prototype's `email` and re-binds it to call the
// original with env/ctx pulled from the WorkerEntrypoint instance.

import Entrypoint from "./build/index.js";

const origEmail = Entrypoint.prototype.email;
Entrypoint.prototype.email = function (message) {
  return origEmail.call(this, message, this.env, this.ctx);
};

export default Entrypoint;
export * from "./build/index.js";
