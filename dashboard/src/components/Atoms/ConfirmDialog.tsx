import { Dialog, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from './Modal';
import { Fragment, ReactNode, useEffect, useState } from 'react';
import Button from './Button';

export interface DialogProps {
  title: string;
  children: ReactNode;
  type: 'info' | 'danger';
  confirmButtonText?: string;
  closeButtonText?: string;
  onConfirm?: () => void | Promise<void>;
  onClose: () => void;
  isOpen: boolean;
  zIndex?: number;
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
  zIndex = 10,
}: DialogProps) {
  const [isLoading, setIsLoading] = useState(false);
  useEffect(() => {
    setIsLoading(false);
  }, [isOpen]);

  return (
    <Transition appear show={isOpen} as={Fragment}>
      <Dialog
        as="div"
        className="relative"
        style={{
          zIndex,
        }}
        onClose={onClose}
      >
        <ModalBackdrop />
        <div className="fixed inset-0 overflow-y-auto drop-shadow-lg">
          <div className="flex min-h-full items-center justify-center">
            <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-6 rounded-2xl border border-gray-faded/30 bg-gray-850 p-12">
              <Dialog.Title
                as="h1"
                className="text-h1 font-extrabold leading-tight tracking-tight text-gray-300"
              >
                {title}
              </Dialog.Title>
              <Dialog.Description
                as="p"
                className="overflow-hidden text-h3 tracking-medium text-gray-300"
              >
                {children}
              </Dialog.Description>
              <div className="flex flex-row gap-2">
                <Button
                  label={closeButtonText || 'Cancel'}
                  className={onConfirm ? 'w-fit' : 'grow'}
                  onClick={onClose}
                  disabled={isLoading}
                />
                {onConfirm && (
                  <Button
                    label={confirmButtonText || 'Confirm'}
                    intention={type === 'danger' ? 'danger' : 'info'}
                    loading={isLoading}
                    onClick={async () => {
                      setIsLoading(true);
                      try {
                        await onConfirm();
                      } finally {
                        // a failed confirm leaves the dialog open, so it must
                        // not stay stuck in the loading state
                        setIsLoading(false);
                      }
                    }}
                  />
                )}
              </div>
            </ModalPanel>
          </div>
        </div>
      </Dialog>
    </Transition>
  );
}
