import { Dialog, Transition } from '@headlessui/react';
import { Fragment, ReactNode } from 'react';
import Button from './Button';

export interface DialogProps {
  title: string;
  children: ReactNode;
  type: 'info' | 'danger';
  confirmButtonText?: string;
  closeButtonText?: string;
  onConfirm?: () => void;
  onClose: () => void;
  isOpen: boolean;
}

export default function ConfirmDialog({
  title,
  children,
  type,
  confirmButtonText,
  closeButtonText,
  onConfirm,
  onClose,
  isOpen,
}: DialogProps) {
  return (
    <Transition appear show={isOpen} as={Fragment}>
      <Dialog as="div" className="relative z-10" onClose={onClose}>
        <Transition.Child
          as={Fragment}
          enter="ease-out duration-300"
          enterFrom="opacity-0"
          enterTo="opacity-100"
          leave="ease-in duration-200"
          leaveFrom="opacity-100"
          leaveTo="opacity-0"
        >
          <div className="fixed inset-0 bg-gray-900/60" />
        </Transition.Child>
        <div className="fixed inset-0 overflow-y-auto">
          <div className="flex min-h-full items-center justify-center">
            <Transition.Child
              as={Fragment}
              enter="ease-out duration-300"
              enterFrom="opacity-0 scale-95"
              enterTo="opacity-100 scale-100"
              leave="ease-in duration-200"
              leaveFrom="opacity-100 scale-100"
              leaveTo="opacity-0 scale-95"
            >
              <Dialog.Panel className="flex w-[500px] flex-col items-stretch justify-center gap-6 rounded-2xl border border-gray-faded/30 bg-gray-850 p-12">
                <Dialog.Title
                  as="h1"
                  className="text-larger font-extrabold tracking-tight leading-tight text-gray-300"
                >
                  {title}
                </Dialog.Title>
                <Dialog.Description
                  as="p"
                  className="text-medium text-gray-300 tracking-medium"
                >
                  {children}
                </Dialog.Description>
                <div className="flex flex-row gap-6">
                  <Button
                    label={closeButtonText || 'Cancel'}
                    className={onConfirm ? 'w-fit' : 'grow'}
                    onClick={onClose}
                  />
                  {onConfirm && (
                    <Button
                      label={confirmButtonText || 'Confirm'}
                      className="grow"
                      color={type === 'danger' ? 'danger' : 'plain'}
                      onClick={onConfirm}
                    />
                  )}
                </div>
              </Dialog.Panel>
            </Transition.Child>
          </div>
        </div>
      </Dialog>
    </Transition>
  );
}
