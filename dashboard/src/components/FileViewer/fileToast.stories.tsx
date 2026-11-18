import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { FileToastContent } from './fileToast';

const meta: Meta<typeof FileToastContent> = {
  title: 'components/FileViewer/FileToast',
  component: FileToastContent,
  decorators: [
    (Story) => (
      <div className="w-80 rounded-md bg-gray-800 p-3">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof FileToastContent>;

export const Deleted: Story = {
  args: {
    file: { name: 'server.properties', kind: 'file' },
    outcome: 'Deleted',
  },
};

export const FolderDeleted: Story = {
  args: {
    file: { name: 'world_nether', kind: 'folder' },
    outcome: 'Deleted',
  },
};

export const UnzipFailed: Story = {
  args: {
    file: {
      name: 'world-backup-2026-11-03-with-a-long-name.zip',
      kind: 'archive',
    },
    outcome: 'Couldn’t unzip',
    reason: 'The archive is damaged: unexpected end of file',
  },
};

export const Unzipping: Story = {
  args: {
    file: { name: 'datapacks.tar.gz', kind: 'archive' },
    outcome: 'Unzipping…',
  },
};
