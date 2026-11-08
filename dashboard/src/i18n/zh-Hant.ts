// TEMPORARY aggregate of parts/, merged into this file at the end.
import type { Messages } from './types.ts';
import common from './parts/common/zh-Hant.ts';
import shell from './parts/shell/zh-Hant.ts';
import settings from './parts/settings/zh-Hant.ts';
import server from './parts/server/zh-Hant.ts';
import create from './parts/create/zh-Hant.ts';
import files from './parts/files/zh-Hant.ts';
import mods from './parts/mods/zh-Hant.ts';
import consoleMessages from './parts/console/zh-Hant.ts';

export const zhHant: Messages = {
  ...common,
  ...shell,
  ...settings,
  ...server,
  ...create,
  ...files,
  ...mods,
  ...consoleMessages,
};
