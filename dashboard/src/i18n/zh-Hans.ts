// TEMPORARY aggregate of parts/, merged into this file at the end.
import type { Messages } from './types.ts';
import common from './parts/common/zh-Hans.ts';
import shell from './parts/shell/zh-Hans.ts';
import settings from './parts/settings/zh-Hans.ts';
import server from './parts/server/zh-Hans.ts';
import create from './parts/create/zh-Hans.ts';
import files from './parts/files/zh-Hans.ts';
import mods from './parts/mods/zh-Hans.ts';

export const zhHans: Messages = {
  ...common,
  ...shell,
  ...settings,
  ...server,
  ...create,
  ...files,
  ...mods,
};
