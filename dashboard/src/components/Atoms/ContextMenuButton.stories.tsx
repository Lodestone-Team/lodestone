import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import ContextMenuButton from './ContextMenuButton';

// The file context menu: the items are verbs, and what they go to (the
// archive or folder name) is a mono caption under the verb
const meta: Meta<typeof ContextMenuButton> = {
  title: 'library/ContextMenuButton',
  component: ContextMenuButton,
  decorators: [
    (Story) => (
      <div className="flex w-56 flex-col gap-0.5 rounded-lg border border-gray-faded/30 bg-gray-900 p-2">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof ContextMenuButton>;

export const Plain: Story = {
  args: { label: 'Rename', className: 'w-full' },
};

export const WithCaption: Story = {
  args: { label: 'Zip', caption: 'world_nether.zip', className: 'w-full' },
};

export const LongCaption: Story = {
  args: {
    label: 'Unzip to a folder',
    caption: 'all-the-mods-9-server-files-0.3.2',
    className: 'w-full',
  },
};

export const WithShortcut: Story = {
  args: { label: 'Copy', subLabel: '⌘C', className: 'w-full' },
};

export const Disabled: Story = {
  args: {
    label: 'Paste into',
    caption: 'config',
    disabled: true,
    className: 'w-full',
  },
};
