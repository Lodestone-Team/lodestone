// Stories file for Button component
import { faServer } from '@fortawesome/free-solid-svg-icons';
import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import Button from './Button';

const meta: Meta<typeof Button> = {
  title: 'library/Button',
  component: Button,
  argTypes: {
    disabled: { control: 'boolean' },
    onClick: { table: { disable: true } },
    className: { table: { disable: true } },
  },
};
export default meta;

type Story = StoryObj<typeof Button>;

export const Default: Story = {
  args: {
    label: 'Button',
  },
};

export const Disabled: Story = {
  args: {
    label: 'Button',
    disabled: true,
  },
};

export const StartServer: Story = {
  args: {
    label: 'Start Server',
    onClick: () => {
      console.log('Start Server');
    },
    icon: faServer,
  },
};
