import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { VersionTag } from 'components/Atoms/Values';
import { CompareTable } from './CompareTable';

const meta: Meta<typeof CompareTable> = {
  title: 'Values/CompareTable',
  component: CompareTable,
  decorators: [
    (Story) => (
      <div className="w-96">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof CompareTable>;

export const IncompatibleVersion: Story = {
  args: {
    columns: ['This version', 'This server'],
    rows: [
      { label: 'Loader', a: 'Forge', b: 'Fabric', mismatch: true },
      { label: 'Minecraft', a: '1.21.1', b: '1.21.1' },
    ],
  },
};

export const CoreVersion: Story = {
  args: {
    columns: ['Core', 'Dashboard'],
    rows: [
      {
        label: 'Version',
        a: <VersionTag version="0.5.0" />,
        b: <VersionTag version="0.5.1" />,
        mismatch: true,
      },
    ],
  },
};
