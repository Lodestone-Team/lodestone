import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { MonoText, VersionTag } from 'components/Atoms/Values';
import { DiffValue } from './DiffValue';

const meta: Meta<typeof DiffValue> = {
  title: 'Values/DiffValue',
  component: DiffValue,
};
export default meta;

type Story = StoryObj<typeof DiffValue>;

export const Versions: Story = {
  args: {
    from: <VersionTag version="0.5.9" tone="from" />,
    to: <VersionTag version="0.6.0" tone="to" />,
  },
};

export const FileNames: Story = {
  args: {
    className: 'text-small text-gray-400',
    from: <MonoText text="sodium-0.5.9.jar" />,
    to: <MonoText text="sodium-0.6.0.jar" />,
  },
};
