import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import InstalledMods from './InstalledMods';
import { fabricList, paperList } from './modStoryFixtures';

// The list comes in as props; the buttons' requests go nowhere in Storybook
const instance: InstanceInfo = {
  uuid: 'smp',
  name: 'Survival SMP',
  game_type: { type: 'MinecraftJava', variant: { type: 'Fabric' } },
  description: '',
  version: '1.21.1',
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
  modpack: null,
  loader_version: '0.16.9',
  min_ram: null,
  max_ram: null,
};

const meta: Meta<typeof InstalledMods> = {
  title: 'components/Mods/InstalledMods',
  component: InstalledMods,
  args: {
    uuid: 'smp',
    instance,
    history: undefined,
    historyError: null,
    canUploadPack: true,
    list: fabricList,
    isLoading: false,
    error: null,
    canWrite: true,
    checkingUpdates: false,
    onCheckUpdates: () => undefined,
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={new QueryClient()}>
        <div className="@container flex w-225 max-w-full flex-col gap-4">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof InstalledMods>;

/** Fabric mods: name + version tag, an update as a from → to diff, the file in mono, Size and Added columns */
export const FabricMods: Story = {};

/** Paper plugins, with an update and "Update all" + count */
export const PaperPlugins: Story = {
  args: {
    list: paperList,
    instance: {
      ...instance,
      game_type: { type: 'MinecraftJava', variant: { type: 'Paper' } },
      version: '1.21.4',
      loader_version: null,
    },
  },
};

/** Read-only: the actions are disabled with their reason */
export const ReadOnly: Story = {
  args: { canWrite: false },
};

/** Narrow: the Size and Added columns fold back into the second line */
export const Narrow: Story = {
  decorators: [
    (Story) => (
      <div className="@container flex w-150 flex-col gap-4">
        <Story />
      </div>
    ),
  ],
};
