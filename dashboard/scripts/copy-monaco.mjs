// Copies the installed monaco-editor build (node_modules/monaco-editor/min/vs)
// to public/monaco/vs, so that @monaco-editor/loader loads monaco from our own
// origin instead of downloading it from the jsdelivr CDN at runtime (see
// src/components/FileViewer/FileEditor.tsx). Run by `npm run dev` and
// `npm run build`; the copy is gitignored.
import { cpSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const dashboardDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(dashboardDir, 'node_modules', 'monaco-editor', 'min', 'vs');
const dest = join(dashboardDir, 'public', 'monaco', 'vs');

rmSync(dest, { recursive: true, force: true });
cpSync(src, dest, { recursive: true });
console.log(`Copied ${src} to ${dest}`);
