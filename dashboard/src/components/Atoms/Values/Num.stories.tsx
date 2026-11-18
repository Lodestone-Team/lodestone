import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { Bytes, Fraction, Num } from './Num';

const meta: Meta<typeof Num> = {
  title: 'Values/Num',
  component: Num,
};
export default meta;

type Story = StoryObj<typeof Num>;

export const Count: Story = { args: { value: 1234 } };

export const WithUnit: Story = { args: { value: 42, unit: 'ms' } };

export const Unknown: Story = { args: { value: null } };

/** A live value keeps its width */
export const FixedWidth: Story = {
  args: { value: '12.4', unit: '%', minWidth: '4rem' },
};

export const Sizes: Story = {
  render: () => (
    <div className="flex flex-col gap-1 text-medium">
      <Bytes bytes={2.5 * 1024 * 1024} />
      <Fraction done={1.2 * 1024 ** 2} total={3.4 * 1024 ** 2} bytes />
      <Fraction done={3} total={20} />
      <Fraction done={null} total={null} />
    </div>
  ),
};
