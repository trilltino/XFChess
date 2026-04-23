// Stub WASM module for development - replace with wasm-pack build output

export default function init() {
  console.warn('[WASM] Using stub xfchess_wasm - real WASM not built');
  return Promise.resolve();
}

export function sign_callback(_fn) {
  console.warn('[WASM] sign_callback not implemented in stub');
}

export function load_tournament(_id) {
  console.warn('[WASM] load_tournament not implemented in stub');
}

export function __wbg_set_wasm(_val) {
  // noop
}
