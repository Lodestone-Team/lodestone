import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { Fraction, MonoText } from 'components/Atoms/Values';
import { FactStrip } from './FactStrip';

const meta: Meta<typeof FactStrip> = {
  title: 'Values/FactStrip',
  component: FactStrip,
};
export default meta;

type Story = StoryObj<typeof FactStrip>;

export const Server: Story = {
  args: {
    items: [
      { label: 'Game', value: 'Minecraft 1.21.4' },
      { label: 'Loader', value: 'Fabric 0.16.9' },
      { label: 'Players', value: <Fraction done={3} total={20} /> },
      {
        label: 'Address',
        value: (
          <MonoText text="play.example.com:25565" className="text-small" />
        ),
      },
    ],
  },
};

/** It wraps rather than truncating */
export const Narrow: Story = {
  ...Server,
  decorators: [
    (Story) => (
      <div className="w-56">
        <Story />
      </div>
    ),
  ],
};
