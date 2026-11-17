import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { KIND_ICONS, KindIcon, ObjectKind } from './KindIcon';

const meta: Meta<typeof KindIcon> = {
  title: 'Values/KindIcon',
  component: KindIcon,
};
export default meta;

type Story = StoryObj<typeof KindIcon>;

export const Folder: Story = { args: { kind: 'folder' } };

export const AllKinds: Story = {
  render: () => (
    <ul className="grid grid-cols-3 gap-x-6 gap-y-1 text-small text-gray-300">
      {(Object.keys(KIND_ICONS) as ObjectKind[]).map((kind) => (
        <li key={kind} className="flex items-center gap-1.5">
          <KindIcon kind={kind} />
          <span aria-hidden="true">{kind}</span>
        </li>
      ))}
    </ul>
  ),
};
