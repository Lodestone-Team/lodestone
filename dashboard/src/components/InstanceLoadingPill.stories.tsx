import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import InstanceLoadingPill from './InstanceLoadingPill';

const meta: Meta<typeof InstanceLoadingPill> = {
  title: 'library/InstanceLoadingPill',
  component: InstanceLoadingPill,
  decorators: [
    (Story) => (
      <div className="flex w-60 flex-col child:w-full">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof InstanceLoadingPill>;

export const Default: Story = {
  args: {
    progress_percent: 0.5,
  },
};
