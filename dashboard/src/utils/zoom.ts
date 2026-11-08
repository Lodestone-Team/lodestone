import { tauri } from 'utils/tauriUtil';

import { ZOOM_LEVELS, nextZoomLevel, zoomShortcut } from './zoomLevels';

// Zoom in the desktop app: Ctrl or ⌘ with + and − zooms the page, and 0 goes
// back to 100%, like a browser. In a browser the browser does this itself, so
// nothing here runs there. The level is kept between launches.

const ZOOM_STORAGE_KEY = 'lodestone-zoom';

const readStoredZoom = (): number => {
  try {
    const stored = Number(window.localStorage.getItem(ZOOM_STORAGE_KEY));
    if (
      stored >= ZOOM_LEVELS[0] &&
      stored <= ZOOM_LEVELS[ZOOM_LEVELS.length - 1]
    )
      return stored;
  } catch {
    // storage blocked
  }
  return 1;
};

let zoom = 1;

const applyZoom = (level: number) => {
  zoom = level;
  try {
    if (level === 1) window.localStorage.removeItem(ZOOM_STORAGE_KEY);
    else window.localStorage.setItem(ZOOM_STORAGE_KEY, String(level));
  } catch {
    // storage blocked: the level lasts until the app quits
  }
  // Needs core:webview:allow-set-webview-zoom.
  tauri?.webview
    ?.getCurrentWebview()
    .setZoom(level)
    .catch((e: unknown) => console.warn('Failed to zoom', e));
};

let initialized = false;

/** Restores the saved zoom and listens for the shortcuts. Call once on the client. */
export const initDesktopZoom = () => {
  if (initialized || typeof window === 'undefined' || !tauri) return;
  initialized = true;
  const saved = readStoredZoom();
  if (saved !== 1) applyZoom(saved);
  window.addEventListener(
    'keydown',
    (event) => {
      const action = zoomShortcut(event);
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      applyZoom(
        action === 'reset' ? 1 : nextZoomLevel(zoom, action === 'in' ? 1 : -1)
      );
    },
    { capture: true }
  );
};
