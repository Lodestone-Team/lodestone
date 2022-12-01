import tauriApi from '@tauri-apps/api';
export type tauri = typeof tauriApi;
declare global {
  interface Window {
    __TAURI__?: tauri;
  }
}
export const tauri = window.__TAURI__;
