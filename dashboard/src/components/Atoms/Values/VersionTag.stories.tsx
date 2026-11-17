import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { VersionTag } from './VersionTag';

const meta: Meta<typeof VersionTag> = {
  title: 'Values/VersionTag',
  component: VersionTag,
  argTypes: { tone: { control: 'inline-radio' } },
};
export default meta;

type Story = StoryObj<typeof VersionTag>;

export const Normal: Story = { args: { version: '0.116.17+1.21.1' } };

/** A noisy Modrinth version, kept verbatim and cut in the middle */
export const Long: Story = {
  args: { version: 'mc1.21.1-0.6.0-beta.2-fabric-build.1234' },
};

export const AfterName: Story = {
  render: () => (
    <span className="flex items-center gap-1.5 text-medium">
      <span className="font-mediumbold text-gray-300">Fabric API</span>
      <VersionTag version="0.116.17+1.21.1" />
    </span>
  ),
};

export const FromTo: Story = {
  render: () => (
    <span className="flex items-center gap-1.5">
      <VersionTag version="6.1.0" tone="from" />
      <span className="text-gray-500">→</span>
      <VersionTag version="6.2.0" tone="to" />
    </span>
  ),
};
