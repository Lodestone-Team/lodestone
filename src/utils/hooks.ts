import { useRouter } from 'next/router';
import { useEffect, useRef, useState } from 'react';

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
 * @param key a unique key to store the value under
 * @param defaultValue the default value to use if there is no value in local storage or the query string
 * @param defaultToLocal if true, the value in local storage will be used if there is no value in the query string
 * @param visible if true, the query string will be updated when the value is changed
 * @returns
 */
export function useRouterQuery(
  key: string,
  defaultValue?: string,
  defaultToLocal = true,
  visible = true
) {
  const router = useRouter();
  const [storage, setStorage] = useLocalStorage<string>(
    `${key}-router-query`,
    defaultValue || ''
  );
  const [ready, setReady] = useState(false);

  const setQuery = (value: string, pathname?: string) => {
    setStorage(value);
    if (visible)
      router.replace(
        {
          pathname: pathname || router.pathname,
          query: { ...router.query, [key]: value },
        },
        undefined,
        { shallow: true }
      );
  };

  useEffect(() => {
    // check if it's an array
    const val = router.query[key];
    let newVal: string | undefined;
    if (!val) {
      newVal = undefined;
    } else if (Array.isArray(val)) {
      newVal = val[0];
    } else {
      newVal = val;
    }

    if (defaultToLocal && !newVal && storage) {
      newVal = storage;
      if (visible) setQuery(newVal);
    } else if (newVal !== storage) {
      setStorage(newVal || '');
    }
  }, [router.query, key]);

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
