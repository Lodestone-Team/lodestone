import { ComponentType, lazy, LazyExoticComponent } from 'react';

// any: React.lazy accepts components with any props, like ComponentType<any>
/* eslint-disable @typescript-eslint/no-explicit-any */

type Module<T> = { default: T };

export type PreloadableComponent<T extends ComponentType<any>> =
  LazyExoticComponent<T> & {
    /** Starts loading the chunk; resolves when it has loaded. */
    preload: () => Promise<Module<T>>;
  };

/**
 * Like React.lazy, with a `preload()` to fetch the chunk ahead of time, for
 * example on hover or as soon as the page that links to it mounts.
 *
 * A plain lazy component suspends on its first render even when its chunk is
 * already loaded (the import() promise only resolves a tick later), so its
 * Suspense fallback flashes. Once preloaded, this one renders right away.
 */
export function lazyWithPreload<T extends ComponentType<any>>(
  factory: () => Promise<Module<T>>
): PreloadableComponent<T> {
  let loaded: Module<T> | undefined;
  let loading: Promise<Module<T>> | undefined;

  const preload = () => {
    loading ??= factory().then(
      (loadedModule) => {
        loaded = loadedModule;
        return loadedModule;
      },
      (error) => {
        // a failed chunk load (e.g. offline) can be retried later
        loading = undefined;
        throw error;
      }
    );
    return loading;
  };

  const Component = lazy(() => {
    // React reads a thenable that resolves synchronously without suspending
    const done = loaded;
    if (done) {
      return {
        then: (resolve: (value: Module<T>) => void) => resolve(done),
      } as unknown as Promise<Module<T>>;
    }
    return preload();
  });

  return Object.assign(Component, { preload });
}
