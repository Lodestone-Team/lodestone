import tauriApi from '@tauri-apps/api';
export type tauri = typeof tauriApi;
declare global {
  interface Window {
    __TAURI__?: tauri;
  }
}

const globalTauriExists: boolean =
  typeof window !== 'undefined' && !!window.__TAURI__ && !!window.__TAURI__.tauri;

export const tauri = globalTauriExists ? window.__TAURI__ : undefined;
