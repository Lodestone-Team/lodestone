import CreateFolderForm from './CreateFolderForm';
import { Dialog } from '@headlessui/react';
import { ClientFile } from 'bindings/ClientFile';

export default function CreateFolderModal({
  modalOpen,
  setModalOpen,
  path,
  fileList,
} : {
  modalOpen: boolean,
  setModalOpen: (modalOpen: boolean) => void,
  path: string,
  fileList: ClientFile[] | undefined
}) {
  return (
    <Dialog
      open={modalOpen}
      onClose={() => setModalOpen(false)}
    >
      <div className="fixed inset-0 bg-[#000]/80" />
      <div className="fixed inset-0 overflow-y-auto">
        <div className="flex min-h-full items-center justify-center p-4 text-center">
          <Dialog.Panel className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-800 px-8 pb-8 pt-16">
            <CreateFolderForm
              onCancel={() => setModalOpen(false)}
              onSuccess={() => setModalOpen(false)}
              path={path}
              fileList={fileList}
            />
          </Dialog.Panel>
        </div>
      </div>
    </Dialog>
  ) 
}
