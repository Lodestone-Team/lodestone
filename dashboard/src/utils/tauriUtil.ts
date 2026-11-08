import { useEffect, useState } from 'react';

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

// True in the macOS desktop app. Its window uses an overlay title bar (see tauri.conf.json:
// titleBarStyle "Overlay"), so the page draws under the traffic-light buttons and has to
// provide its own drag regions and leave room for the buttons.
export const isTauriMacOS: boolean =
  globalTauriExists && /Mac/i.test(window.navigator.userAgent);

// Sets the desktop window's light or dark appearance (on macOS the title bar buttons drawn
// over the page), or null to follow the OS again. Needs core:window:allow-set-theme.
export const setTauriWindowTheme = (theme: 'light' | 'dark' | null) => {
  tauri?.window
    ?.getCurrentWindow()
    .setTheme(theme)
    .catch((e: unknown) => console.warn('Failed to set the window theme', e));
};

// The desktop app's answer to is_debug_build, kept for the page's lifetime:
// asking again on every mount showed the release look until the answer came.
let tauriDebugBuildCache: boolean | undefined;

// True for local development: the Next dev server, or a debug build of the desktop app.
export const useIsDevBuild = (): boolean => {
  const [tauriDebugBuild, setTauriDebugBuild] = useState(
    tauriDebugBuildCache ?? false
  );
  useEffect(() => {
    if (tauriDebugBuildCache !== undefined) return;
    tauri?.core
      .invoke<boolean>('is_debug_build')
      .then((debug) => {
        tauriDebugBuildCache = debug;
        setTauriDebugBuild(debug);
      })
      .catch(() => setTauriDebugBuild(false));
  }, []);
  return process.env.NODE_ENV === 'development' || tauriDebugBuild;
};
