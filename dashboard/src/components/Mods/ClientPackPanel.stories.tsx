import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import ClientPackPanel from './ClientPackPanel';
import { fabricList, fabricMods } from './modStoryFixtures';

const meta: Meta<typeof ClientPackPanel> = {
  title: 'components/Mods/ClientPackPanel',
  component: ClientPackPanel,
  decorators: [
    (Story) => (
      <div className="w-180 max-w-full">
        <Story />
      </div>
    ),
  ],
  args: { uuid: 'smp' },
};
export default meta;

type Story = StoryObj<typeof ClientPackPanel>;

/** Players need two mods: the title with a count, the Game/Loader strip, chips with version tags */
export const PlayersNeedMods: Story = {
  args: { list: fabricList },
};

/** A big pack folds after its first few chips, with a counted "Show N more" */
export const ManyMods: Story = {
  args: {
    list: {
      ...fabricList,
      client_required_count: 20,
      mods: Array.from({ length: 20 }, (_, i) => ({
        ...fabricMods[1],
        file_name: `mod-${i}.jar`,
        metadata: {
          ...fabricMods[1].metadata!,
          name: `Example mod ${i + 1}`,
          version: `1.${i}.0`,
        },
        modrinth: null,
      })),
    },
  },
};

/** Players need nothing but Fabric: one quiet line with the two phrases */
export const PlainFabric: Story = {
  args: { list: { ...fabricList, client_required_count: 0 } },
};
