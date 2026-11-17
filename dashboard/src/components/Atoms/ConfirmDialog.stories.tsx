import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import GameIcon from 'components/Atoms/GameIcon';
import { StatusBadge } from 'components/Atoms/Values';
import { FactLine, ObjectCard } from 'components/Values';
import ConfirmDialog from './ConfirmDialog';

const meta: Meta<typeof ConfirmDialog> = {
  title: 'Library/ConfirmDialog',
  component: ConfirmDialog,
  args: { isOpen: true, onClose: () => {} },
};
export default meta;

type Story = StoryObj<typeof ConfirmDialog>;

/** The title names the action; the object is a card under it */
export const DeleteServer: Story = {
  args: {
    title: 'Delete server?',
    type: 'danger',
    confirmButtonText: 'Delete',
    onConfirm: () => {},
    object: (
      <ObjectCard
        icon={
          <GameIcon
            game_type={{ type: 'MinecraftJava', variant: { type: 'Fabric' } }}
            className="size-8"
          />
        }
        name="Survival SMP"
        facts={
          <FactLine>
            <span>Minecraft 1.21.4</span>
            <span>Fabric 0.16.9</span>
          </FactLine>
        }
        badge={<StatusBadge tone="neutral">Stopped</StatusBadge>}
      />
    ),
    children: (
      <>
        <b>This can’t be undone.</b> Its settings, worlds and backups are
        deleted for good.
      </>
    ),
  },
};
