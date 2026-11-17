import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { Num } from 'components/Atoms/Values';
import { StatRow, StatTile } from './StatTile';

const meta: Meta<typeof StatTile> = {
  title: 'Values/StatTile',
  component: StatTile,
};
export default meta;

type Story = StoryObj<typeof StatTile>;

export const CpuLoad: Story = {
  args: {
    label: 'CPU load',
    value: <Num value="12.4" unit="%" minWidth="4.5rem" />,
  },
};

export const Row: Story = {
  render: () => (
    <StatRow>
      <StatTile label="Needed" value={<Num value="2.4" unit="GB" />} />
      <StatTile
        label="Free"
        tone="danger"
        value={<Num value="1.1" unit="GB" />}
      />
    </StatRow>
  ),
};
