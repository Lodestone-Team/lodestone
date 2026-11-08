// TEMPORARY aggregate of parts/, merged into this file at the end.
import type { Messages } from './types.ts';
import common from './parts/common/fr.ts';
import shell from './parts/shell/fr.ts';
import settings from './parts/settings/fr.ts';
import server from './parts/server/fr.ts';
import create from './parts/create/fr.ts';
import files from './parts/files/fr.ts';
import mods from './parts/mods/fr.ts';
import consoleMessages from './parts/console/fr.ts';

export const fr: Messages = {
  ...common,
  ...shell,
  ...settings,
  ...server,
  ...create,
  ...files,
  ...mods,
  ...consoleMessages,
};
