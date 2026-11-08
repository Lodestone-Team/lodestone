// The English catalog: the source of every key (see types.ts).
// TEMPORARY aggregate of parts/, merged into this file at the end.
import common from './parts/common/en.ts';
import shell from './parts/shell/en.ts';
import settings from './parts/settings/en.ts';
import server from './parts/server/en.ts';
import create from './parts/create/en.ts';
import files from './parts/files/en.ts';
import mods from './parts/mods/en.ts';

export const en = {
  ...common,
  ...shell,
  ...settings,
  ...server,
  ...create,
  ...files,
  ...mods,
} as const;
