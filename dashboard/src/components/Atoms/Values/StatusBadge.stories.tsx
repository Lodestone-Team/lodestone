import { faTriangleExclamation } from '@fortawesome/free-solid-svg-icons';
import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { StatusBadge } from './StatusBadge';

const meta: Meta<typeof StatusBadge> = {
  title: 'Values/StatusBadge',
  component: StatusBadge,
  argTypes: { tone: { control: 'inline-radio' } },
};
export default meta;

type Story = StoryObj<typeof StatusBadge>;

export const Running: Story = {
  args: { tone: 'success', children: 'Running' },
};

export const Expired: Story = { args: { tone: 'danger', children: 'Expired' } };

export const AllTones: Story = {
  render: () => (
    <div className="flex gap-2">
      <StatusBadge tone="info">Setting up</StatusBadge>
      <StatusBadge tone="success">Running</StatusBadge>
      <StatusBadge tone="warning" icon={faTriangleExclamation}>
        Not for this server
      </StatusBadge>
      <StatusBadge tone="danger">Error</StatusBadge>
      <StatusBadge tone="neutral">Stopped</StatusBadge>
    </div>
  ),
};
