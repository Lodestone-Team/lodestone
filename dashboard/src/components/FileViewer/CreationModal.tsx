import { Dialog } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';

export default function CreateFolderModal({
  modalOpen,
  setModalOpen,
  children,
} : {
  modalOpen: boolean,
  setModalOpen: (modalOpen: boolean) => void,
  children: React.ReactNode
}) {
  return (
    <Dialog
      open={modalOpen}
      onClose={() => setModalOpen(false)}
    >
      <ModalBackdrop strong />
      <div className="fixed inset-0 overflow-y-auto">
        <div className="flex min-h-full items-center justify-center p-4 text-center">
          <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-800 px-8 pb-8 pt-16">
            {children}
          </ModalPanel>
        </div>
      </div>
    </Dialog>
  ) 
}