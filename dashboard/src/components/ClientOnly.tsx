import { ReactNode, useSyncExternalStore } from 'react';

const subscribe = () => () => {};

/**
 * Renders its children only on the client (never during the static export
 * pre-render), like the former react-no-ssr wrapper.
 */
export default function ClientOnly({ children }: { children: ReactNode }) {
  const isClient = useSyncExternalStore(
    subscribe,
    () => true,
    () => false
  );
  return isClient ? children : null;
}
