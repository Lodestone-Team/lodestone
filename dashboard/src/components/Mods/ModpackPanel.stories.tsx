import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { ModpackHistory } from 'bindings/ModpackHistory';
import ModpackPanel from './ModpackPanel';

const hour = 60 * 60 * 1000;
const day = 24 * hour;
const now = Date.now();

const instance: InstanceInfo = {
  uuid: 'smp',
  name: 'Survival SMP',
  game_type: { type: 'MinecraftJava', variant: { type: 'Fabric' } },
  description: '',
  version: '1.21.4',
  port: 25565,
  creation_time: BigInt(0),
  path: '',
  auto_start: false,
  restart_on_crash: false,
  state: 'Stopped',
  player_count: null,
  max_player_count: null,
  player_list: null,
  icon_url: null,
  modpack: {
    project_id: '1KVo5zza',
    version_id: 'fo-640',
    name: 'Fabulously Optimized',
    version_number: '6.4.0',
    icon_url: null,
  },
  loader_version: '0.16.10',
  min_ram: null,
  max_ram: null,
};

const history: ModpackHistory = {
  entries: [
    {
      id: 3,
      time: now - 2 * hour,
      kind: { type: 'update' },
      name: 'Fabulously Optimized',
      version_number: '6.4.0',
      game_version: '1.21.4',
      loader: 'fabric',
      loader_version: '0.16.10',
      current: true,
      file_count: 64,
      missing: 0,
      kept: ['config/sodium-options.json'],
      changelog:
        '## 6.4.0\n\n- Updated **Sodium** to 0.6.9\n- Added Entity Culling\n- Removed Starlight, which Sodium no longer needs',
      changes: [
        {
          kind: 'added',
          path: 'mods/entityculling-1.7.3.jar',
          old_path: null,
          project_id: 'ec',
        },
        {
          kind: 'updated',
          path: 'mods/sodium-0.6.9.jar',
          old_path: 'mods/sodium-0.6.6.jar',
          project_id: 'so',
        },
        {
          kind: 'updated',
          path: 'mods/lithium-0.15.0.jar',
          old_path: 'mods/lithium-0.14.7.jar',
          project_id: 'li',
        },
        {
          kind: 'replaced',
          path: 'config/modmenu.json',
          old_path: null,
          project_id: null,
        },
        {
          kind: 'removed',
          path: 'mods/starlight-1.1.3.jar',
          old_path: null,
          project_id: 'st',
        },
      ],
    },
    {
      id: 2,
      time: now - 9 * day,
      kind: { type: 'update' },
      name: 'Fabulously Optimized',
      version_number: '6.3.1',
      game_version: '1.21.4',
      loader: 'fabric',
      loader_version: '0.16.9',
      current: false,
      file_count: 63,
      missing: 0,
      kept: [],
      changelog: null,
      changes: [
        {
          kind: 'updated',
          path: 'mods/sodium-0.6.6.jar',
          old_path: 'mods/sodium-0.6.5.jar',
          project_id: 'so',
        },
        {
          kind: 'added',
          path: 'mods/starlight-1.1.3.jar',
          old_path: null,
          project_id: 'st',
        },
      ],
    },
    {
      id: 1,
      time: now - 30 * day,
      kind: { type: 'install' },
      name: 'Fabulously Optimized',
      version_number: '6.2.0',
      game_version: '1.21.3',
      loader: 'fabric',
      loader_version: '0.16.7',
      current: false,
      file_count: 61,
      missing: 2,
      kept: [],
      changelog: null,
      changes: [],
    },
  ],
  blame: {},
  stored_bytes: 48_213,
  update: {
    version_id: 'fo-650',
    version_number: '6.5.0',
    name: '6.5.0',
    game_versions: ['1.21.4'],
    date_published: '2026-11-10T00:00:00Z',
    changelog: '## 6.5.0\n\n- Minecraft 1.21.4 hotfixes\n- Lithium 0.15.1',
  },
};

/** The modpack panel of a server's Mods tab, with its history open. */
const meta: Meta<typeof ModpackPanel> = {
  title: 'components/Mods/ModpackPanel',
  component: ModpackPanel,
  args: {
    instance,
    history,
    historyError: null,
    canWrite: true,
    canUpload: true,
    initialHistoryOpen: true,
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={new QueryClient()}>
        <div className="w-225">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof ModpackPanel>;

export const WithHistory: Story = {};

/** Up to date, history closed. */
export const Closed: Story = {
  args: {
    history: { ...history, update: null },
    initialHistoryOpen: false,
  },
};

/** A server made before histories: nothing recorded yet. */
export const NoHistory: Story = {
  args: {
    history: { entries: [], blame: {}, stored_bytes: 0, update: null },
  },
};
