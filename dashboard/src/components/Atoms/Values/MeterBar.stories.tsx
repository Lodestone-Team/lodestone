import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { MeterBar } from './MeterBar';

const meta: Meta<typeof MeterBar> = {
  title: 'Values/MeterBar',
  component: MeterBar,
  decorators: [
    (Story) => (
      <div className="w-64">
        <Story />
      </div>
    ),
  ],
  args: { label: 'Memory used' },
};
export default meta;

type Story = StoryObj<typeof MeterBar>;

export const Half: Story = { args: { value: 2, max: 4 } };

export const Warning: Story = { args: { value: 3.6, max: 4, tone: 'warning' } };

/** A marker shows what is needed against what there is */
export const WithMarker: Story = {
  args: { value: 1.1, max: 4, marker: 2.4, tone: 'danger' },
};
