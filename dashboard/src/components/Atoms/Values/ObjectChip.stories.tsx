import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { ObjectChip } from './ObjectChip';

const meta: Meta<typeof ObjectChip> = {
  title: 'Values/ObjectChip',
  component: ObjectChip,
  args: { className: 'text-medium' },
};
export default meta;

type Story = StoryObj<typeof ObjectChip>;

export const Server: Story = { args: { kind: 'server', name: 'Survival SMP' } };

export const File: Story = {
  args: { kind: 'file', name: 'server.properties', mono: true },
};

/** A long name is cut, with the full name in the title */
export const Long: Story = {
  args: {
    kind: 'mod',
    name: 'Fabulously Optimized Extra Long Modpack Name For Testing',
    max: '10rem',
  },
};

export const Muted: Story = {
  args: { kind: 'core', name: 'Home core', muted: true },
};
