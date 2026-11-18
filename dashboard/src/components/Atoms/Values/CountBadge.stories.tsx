import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { CountBadge } from './CountBadge';

const meta: Meta<typeof CountBadge> = {
  title: 'Values/CountBadge',
  component: CountBadge,
};
export default meta;

type Story = StoryObj<typeof CountBadge>;

export const Small: Story = { args: { count: 7 } };

/** Compact, with the exact count in the title */
export const Large: Story = { args: { count: 12345 } };

export const Active: Story = { args: { count: 3, active: true } };

export const AfterHeading: Story = {
  render: () => (
    <h2 className="flex items-center gap-1.5 text-h3 font-bold text-gray-300">
      History <CountBadge count={7} />
    </h2>
  ),
};
