import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import Label from './Label';

const meta: Meta<typeof Label> = {
  title: 'Library/Label',
  component: Label,
  argTypes: {
    color: { control: 'inline-radio' },
    size: { control: 'inline-radio' },
    children: { control: 'text' },
  },
};
export default meta;

type Story = StoryObj<typeof Label>;

export const RunningCard: Story = {
  args: {
    children: 'Running',
    color: 'green',
  },
};

export const CrashedPage: Story = {
  args: {
    children: 'Crashed',
    color: 'red',
    size: 'large',
  },
};
