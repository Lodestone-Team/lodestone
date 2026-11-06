// This is the only place where Tauri API import is allowed
// eslint-disable-next-line no-restricted-imports
import type * as tauriApi from '@tauri-apps/api'; // Import the Tauri API types (the module has no default export)

export type tauri = typeof tauriApi; // Export the Tauri API as the `tauri` type

declare global {
  interface Window {
    __TAURI__?: tauri; // Declare a global `Window` interface that includes a `__TAURI__` property of type `tauri`
  }
}

const globalTauriExists: boolean =
  typeof window !== 'undefined' && !!window.__TAURI__?.core; // Check whether the Tauri API is available on the `window` object (Tauri 2: window.__TAURI__.core.invoke)

export const tauri = globalTauriExists ? window.__TAURI__ : undefined; // Export the `__TAURI__` object from the `window` object if it exists, or `undefined` if it doesn't

export const useTauri = (): tauri | undefined => tauri; // Export a hook that returns the `tauri` object
