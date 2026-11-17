import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { Bytes, Num, VersionTag } from 'components/Atoms/Values';
import { DiffValue } from './DiffValue';
import { FactList } from './FactList';

const meta: Meta<typeof FactList> = {
  title: 'Values/FactList',
  component: FactList,
  decorators: [
    (Story) => (
      <div className="w-96">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof FactList>;

export const ModpackUpdate: Story = {
  args: {
    framed: true,
    rows: [
      {
        label: 'Fabulously Optimized',
        value: (
          <DiffValue
            from={<VersionTag version="6.1.0" tone="from" />}
            to={<VersionTag version="6.2.0" tone="to" />}
          />
        ),
      },
      { label: 'Game', value: 'Minecraft 1.21.1' },
      { label: 'Loader', value: 'Fabric 0.16.9' },
      { label: 'Added', value: <Num value={3} /> },
      { label: 'Download', value: <Bytes bytes={38 * 1024 ** 2} /> },
    ],
  },
};

export const Compact: Story = {
  args: {
    compact: true,
    rows: [
      { label: 'Version', value: <VersionTag version="0.5.1" /> },
      { label: 'Uptime', value: <Num value="3d 4h" /> },
    ],
  },
};
