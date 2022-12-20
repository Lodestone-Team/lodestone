import { useRouter } from 'next/router';
import { useCallback, useEffect, useRef, useState } from 'react';

import { useIsomorphicLayoutEffect, useLocalStorage } from 'usehooks-ts';

export function useIntervalImmediate(
  callback: () => void,
  delay: number | null
) {
  // Based on usehooks-ts's useInterval
  const savedCallback = useRef(callback);

  // Remember the latest callback if it changes.
  useIsomorphicLayoutEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  // Set up the interval.
  useEffect(() => {
    // Don't schedule if no delay is specified.
    // Note: 0 is a valid value for delay.
    if (!delay && delay !== 0) {
      return;
    }

    const id = setInterval(() => savedCallback.current(), delay);

    return () => clearInterval(id);
  }, [delay]);

  // Run the callback immediately.
  useEffect(() => {
    savedCallback.current();
  }, []);
}

export function useIntervalClock(
  callback: () => void,
  delay: number | null,
  initialDelayGen: () => number
) {
  // Based on usehooks-ts's useInterval
  const savedCallback = useRef(callback);

  // Remember the latest callback if it changes.
  useIsomorphicLayoutEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  // Set up the interval.
  useEffect(() => {
    // Don't schedule if no delay is specified.
    // Note: 0 is a valid value for delay.
    if (!delay && delay !== 0) {
      return;
    }

    const initialDelay = initialDelayGen();

    // call the callback after the initial delay, then set up the interval
    // clear both the timeout and the interval on unmount
    let intervalId: NodeJS.Timeout;
    const timeoutId = setTimeout(() => {
      savedCallback.current();
      intervalId = setInterval(() => savedCallback.current(), delay);
    }, initialDelay);

    return () => {
      clearTimeout(timeoutId);
      clearInterval(intervalId);
    };
  }, [delay, initialDelayGen]);

  // Run the callback immediately.
  useEffect(() => {
    savedCallback.current();
  }, []);
}

export function useIntervalClockSeconds(callback: () => void, seconds: number) {
  const delay = seconds * 1000;
  const initialDelayGen = () => {
    const now = new Date();
    return delay - (now.getTime() % delay);
  };
  useIntervalClock(callback, delay, initialDelayGen);
}

/**
 * Uses both local storage and the query string (example: ?query=value) to store a value.
 * @param localKey a unique key to store the value under in local storage
 * @param defaultValue the default value to use if there is no value in local storage or the query string
 * @param defaultToLocal if true, the value in local storage will be used if there is no value in the query string
 * @param visible if true, the query string will be updated when the value is changed
 * @returns
 */
export function useRouterQuery(
  localKey: string,
  defaultValue: Record<string, string | undefined>,
  defaultToLocal = true
) {
  const router = useRouter();
  const isReady = router.isReady;
  const [storage, setStorage] = useLocalStorage<
    Record<string, string | undefined>
  >(`${localKey}-router-query`, defaultValue || {});
  const [ready, setReady] = useState(false);

  /**
   * Sets the values in local storage and router query
   */
  const setQuery = useCallback(
    (value: Record<string, string | undefined>, pathname?: string) => {
      setStorage(value);
      console.log('setQuery', localKey, value, pathname);
      // replace every key in the query string with the new value
      const newQuery = { ...router.query };
      Object.keys(value).forEach((key) => {
        newQuery[key] = value[key];
      });
      console.log('newQuery', newQuery);

      router.replace(
        {
          pathname: pathname || router.pathname,
          query: newQuery,
        },
        undefined,
        { shallow: true }
      );
    },
    [localKey, router, setStorage]
  );

  /**
   * Watches for changes in the query string and updates the local storage if necessary
   */
  useEffect(() => {
    // reconstruct the object from the query
    const newValue = storage;
    let changed = false;
    if (!isReady) return;
    Object.keys(defaultValue).forEach((key) => {
      console.log(router.query);
      if (router.query[key]) {
        const rawVal = router.query[key];
        console.log('rawVal', key, rawVal);
        let val: string | undefined;
        if (!rawVal) {
          val = undefined;
        } else if (Array.isArray(rawVal)) {
          val = rawVal[0];
        } else {
          val = rawVal;
        }
        if (defaultToLocal && !val && storage[key]) {
          changed = true;
          val = storage[key];
        }
        if (val !== storage[key]) {
          changed = true;
          newValue[key] = val;
        }
      } else if (defaultToLocal && storage[key]) {
        changed = true;
        newValue[key] = storage[key];
      }
    });
    console.log(newValue, storage, changed);
    if (changed) {
      setQuery(newValue);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ //dependencies left out on purpose
    router.query,
    isReady,
  ]);

  useEffect(() => {
    setReady(router.isReady);
  }, [router.isReady]);

  return {
    isReady: ready,
    query: storage,
    setQuery,
  };
}

export function usePrevious(value: unknown) {
  const ref = useRef(value);
  useEffect(() => {
    ref.current = value;
  });
  return ref.current;
}
