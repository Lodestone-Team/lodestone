import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import Checkbox from './Checkbox';

const meta: Meta<typeof Checkbox> = {
  title: 'library/Checkbox',
  component: Checkbox,
  argTypes: {
    onChange: { action: 'onChange' },
  },
};
export default meta;

type Story = StoryObj<typeof Checkbox>;

export const Default: Story = {
  args: {
    label: 'Checkbox',
    checked: false,
    disabled: false,
  },
};

export const Checked: Story = {
  args: {
    label: 'Checkbox',
    checked: true,
    disabled: false,
  },
};

export const Disabled: Story = {
  args: {
    label: 'Checkbox',
    checked: false,
    disabled: true,
  },
};
