// Example mods for the Mods stories (InstalledMods, ClientPackPanel).
import { InstalledMod } from 'bindings/InstalledMod';
import { InstalledModList } from 'bindings/InstalledModList';

const mod = (
  fileName: string,
  name: string,
  version: string,
  extra: Partial<InstalledMod> = {},
  modrinth?: Partial<NonNullable<InstalledMod['modrinth']>>
): InstalledMod => ({
  file_name: fileName,
  enabled: true,
  size: 512 * 1024,
  sha512: fileName,
  metadata: {
    id: name.toLowerCase().replace(/\W+/g, '-'),
    name,
    version,
    description: null,
    authors: [],
  },
  modrinth: modrinth
    ? {
        project_id: `p-${fileName}`,
        version_id: `v-${fileName}`,
        slug: name.toLowerCase().replace(/\W+/g, '-'),
        title: name,
        icon_url: null,
        version_number: version,
        update: null,
        client_side: 'optional',
        server_side: 'required',
        ...modrinth,
      }
    : null,
  client_requirement: 'unknown',
  required_by: [],
  client_marked: false,
  client_excluded: false,
  client_optional: false,
  ...extra,
});

export const fabricMods: InstalledMod[] = [
  mod(
    'fabric-api-0.116.17+1.21.1.jar',
    'Fabric API',
    '0.116.17+1.21.1',
    {
      size: 2_621_440,
      client_requirement: 'required',
      required_by: ['Sodium Extra', 'Iris Shaders', 'Mod Menu'],
    },
    { client_side: 'required' }
  ),
  mod(
    'sodium-fabric-0.6.0+mc1.21.1.jar',
    'Sodium',
    'mc1.21.1-0.6.0-fabric',
    { size: 1_048_576, client_requirement: 'required' },
    {
      client_side: 'required',
      update: {
        version_id: 'v-sod-061',
        version_number: 'mc1.21.1-0.6.1-fabric',
      },
    }
  ),
  mod(
    'lithium-fabric-0.15.0+mc1.21.1.jar',
    'Lithium',
    'mc1.21.1-0.15.0-fabric',
    { size: 786_432, client_optional: true },
    { client_side: 'optional' }
  ),
  mod(
    'chunky-1.4.27.jar',
    'Chunky',
    '1.4.27',
    { size: 401_408, client_requirement: 'unsupported' },
    { client_side: 'unsupported' }
  ),
  mod('server-only-tweaks.jar.disabled', 'Server Tweaks', '2.0.0', {
    enabled: false,
    size: 20_480,
  }),
];

export const fabricList: InstalledModList = {
  kind: 'mod',
  loader: 'fabric',
  game_version: '1.21.1',
  directory: 'mods',
  restart_required: false,
  loader_version: '0.16.9',
  client_required_count: 2,
  mods: fabricMods,
};

export const paperList: InstalledModList = {
  kind: 'plugin',
  loader: 'paper',
  game_version: '1.21.4',
  directory: 'plugins',
  restart_required: true,
  loader_version: null,
  client_required_count: 0,
  mods: [
    mod(
      'EssentialsX-2.21.0.jar',
      'EssentialsX',
      '2.21.0',
      { size: 3_145_728 },
      {
        update: { version_id: 'v-ess-2211', version_number: '2.21.1' },
      }
    ),
    mod('LuckPerms-Bukkit-5.4.141.jar', 'LuckPerms', '5.4.141', {
      size: 1_572_864,
    }),
  ],
};
