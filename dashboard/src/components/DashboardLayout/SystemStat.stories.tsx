import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import SystemStat from './SystemStat';

const meta: Meta<typeof SystemStat> = {
  title: 'library/SystemStat',
  component: SystemStat,
};
export default meta;

type Story = StoryObj<typeof SystemStat>;

export const Default: Story = {
  args: {
    name: 'Memory',
    value: '100%',
  },
};
