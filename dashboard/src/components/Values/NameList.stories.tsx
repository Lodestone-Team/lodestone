import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { KindIcon, MonoText, VersionTag } from 'components/Atoms/Values';
import { NameList } from './NameList';

const meta: Meta<typeof NameList> = {
  title: 'Values/NameList',
  component: NameList,
  decorators: [
    (Story) => (
      <div className="w-96">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof NameList>;

export const InstallResult: Story = {
  args: {
    groups: [
      {
        heading: 'Installed',
        tone: 'success',
        items: [
          {
            id: 'sodium',
            name: (
              <span className="flex items-center gap-1.5">
                Sodium <VersionTag version="0.6.0" />
              </span>
            ),
          },
        ],
      },
      {
        heading: 'Skipped',
        items: [
          {
            id: 'iris',
            name: <MonoText text="YL57xq9U" className="text-small" />,
            reason: 'Already installed',
          },
        ],
      },
    ],
  },
};

/** Five rows, then a counted "Show N more" */
export const Collapsed: Story = {
  args: {
    groups: [
      {
        heading: 'Delete',
        items: Array.from({ length: 9 }, (_, i) => ({
          id: String(i),
          icon: <KindIcon kind={i % 3 ? 'file' : 'folder'} />,
          name: <MonoText text={`world_${i}`} className="text-small" />,
        })),
      },
    ],
  },
};
