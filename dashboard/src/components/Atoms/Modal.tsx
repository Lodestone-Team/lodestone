import { DialogBackdrop, DialogPanel } from '@headlessui/react';
import clsx from 'clsx';
import { ReactNode } from 'react';

/*
 * The backdrop and panel every modal uses, so they all open and close with the
 * same motion: the backdrop's dim fades in, and the panel fades in while it
 * slides down 12px into place (200ms ease-out). Closing is the reverse, faster
 * (150ms ease-in). With reduced motion the panel only fades.
 *
 * They animate through headless UI's `transition` prop, so the enclosing
 * `Dialog` (or the `Transition` around it) stays mounted until they finish.
 */

/** The dimmed backdrop. Its colour stays the same, only its opacity animates. */
export function ModalBackdrop({
  strong = false,
}: {
  /** the darker scrim, for modals that cover a busy page */
  strong?: boolean;
}) {
  return (
    <DialogBackdrop
      transition
      className={clsx(
        'fixed inset-0 transition-opacity data-closed:opacity-0',
        'duration-200 ease-out data-leave:duration-150 data-leave:ease-in',
        strong ? 'bg-scrim-strong' : 'bg-scrim'
      )}
    />
  );
}

/** The modal's panel, which slides down from 12px above while it fades in. */
export function ModalPanel({
  className,
  children,
}: {
  className?: string;
  children?: ReactNode;
}) {
  return (
    <DialogPanel
      transition
      className={clsx(
        'transition-[opacity,translate] data-closed:-translate-y-3 data-closed:opacity-0',
        'duration-200 ease-out data-leave:duration-150 data-leave:ease-in',
        'motion-reduce:transition-opacity motion-reduce:data-closed:translate-y-0',
        className
      )}
    >
      {children}
    </DialogPanel>
  );
}
