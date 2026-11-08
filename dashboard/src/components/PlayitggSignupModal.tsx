import { Dialog } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from './Atoms/Modal';

export default function PlayitggSignupModal({
  modalOpen,
  setModalOpen,
  children,
}: {
  modalOpen: boolean;
  setModalOpen: (modalOpen: boolean) => void;
  children: React.ReactNode;
}) {
  return (
    <Dialog
      open={modalOpen}
      onClose={() => setModalOpen(false)}
      className="relative z-60"
    >
      <ModalBackdrop strong />
      <div className="fixed inset-0 overflow-y-auto">
        <div className="flex min-h-full items-center justify-center p-4">
          <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center rounded-3xl bg-gray-800 px-8 pt-16 pb-8">
            {children}
          </ModalPanel>
        </div>
      </div>
    </Dialog>
  );
}
