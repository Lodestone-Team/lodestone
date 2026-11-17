import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { ObjectChip, VersionTag } from 'components/Atoms/Values';
import { NameList } from './NameList';
import { ObjectToast } from './ObjectToast';

const meta: Meta<typeof ObjectToast> = {
  title: 'Values/ObjectToast',
  component: ObjectToast,
  decorators: [
    (Story) => (
      <div className="w-80 rounded-md bg-gray-800 p-3">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof ObjectToast>;

export const Updated: Story = {
  args: {
    object: (
      <>
        <ObjectChip kind="mod" name="Sodium" />
        <VersionTag version="0.6.0" tone="to" />
      </>
    ),
    outcome: 'Updated',
  },
};

export const Failed: Story = {
  args: {
    object: <ObjectChip kind="mod" name="Sodium" />,
    outcome: 'Couldn’t update',
    reason: 'HTTP 404: the file is gone from Modrinth',
  },
};

export const WithList: Story = {
  args: {
    outcome: 'Some updates failed',
    children: (
      <NameList
        groups={[
          {
            heading: 'Failed',
            tone: 'danger',
            items: [
              { id: 'a', name: 'Sodium', reason: 'HTTP 404' },
              { id: 'b', name: 'Iris', reason: 'Connection reset' },
            ],
          },
        ]}
      />
    ),
  },
};
