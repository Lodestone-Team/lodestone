import { Dialog } from '@headlessui/react';

export default function PlayitggSignupModal({
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
      <div className="fixed inset-0 bg-[#000]/80" />
      <div className="fixed inset-0 overflow-y-auto">
        <div className="flex min-h-full items-center justify-center p-4">
          <Dialog.Panel className="flex w-[500px] flex-col items-stretch justify-center rounded-3xl bg-gray-800 px-8 pb-8 pt-16">
            {children}
          </Dialog.Panel>
        </div>
      </div>
    </Dialog>
  ) 
}