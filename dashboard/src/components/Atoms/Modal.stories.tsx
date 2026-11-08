import { Dialog } from '@headlessui/react';
import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { useEffect, useState } from 'react';
import Button from './Button';
import ConfirmDialog from './ConfirmDialog';
import { ModalBackdrop, ModalPanel } from './Modal';

/** The modal motion, in a theme set on the root (dialogs open in a portal,
 * outside any wrapper). Open each one and close it again to see both ways. */
const Demo = ({ theme }: { theme: 'dark' | 'light' }) => {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [plainOpen, setPlainOpen] = useState(false);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);
  return (
    <div className="flex flex-row gap-2 rounded-lg bg-gray-800 p-4">
      <Button label="Confirm dialog" onClick={() => setConfirmOpen(true)} />
      <Button label="Plain modal" onClick={() => setPlainOpen(true)} />
      <ConfirmDialog
        title="Delete server?"
        type="danger"
        isOpen={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => setConfirmOpen(false)}
      >
        This can&apos;t be undone.
      </ConfirmDialog>
      <Dialog open={plainOpen} onClose={() => setPlainOpen(false)}>
        <ModalBackdrop strong />
        <div className="fixed inset-0 overflow-y-auto">
          <div className="flex min-h-full items-center justify-center p-4">
            <ModalPanel className="flex w-125 flex-col gap-4 rounded-3xl bg-gray-800 p-8">
              <p className="text-h3 text-gray-300">
                A modal on the strong scrim
              </p>
              <Button label="Close" onClick={() => setPlainOpen(false)} />
            </ModalPanel>
          </div>
        </div>
      </Dialog>
    </div>
  );
};

const meta: Meta<typeof Demo> = {
  title: 'Library/Modal',
  component: Demo,
};

export default meta;
type Story = StoryObj<typeof Demo>;

export const Dark: Story = { args: { theme: 'dark' } };
export const Light: Story = { args: { theme: 'light' } };
