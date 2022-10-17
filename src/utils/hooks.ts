import { useRouter } from 'next/router';
import { useEffect, useRef, useState } from 'react';

import { useIsomorphicLayoutEffect } from 'usehooks-ts';

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

export function useIntervalClockSeconds(
  callback: () => void,
  seconds: number
) {
  const delay = seconds * 1000;
  const initialDelayGen = () => {
    const now = new Date();
    return delay - (now.getTime() % delay);
  }
  useIntervalClock(callback, delay, initialDelayGen);
}

export function useRouterQuery(queryString: string) {
  const router = useRouter();
  const [state, setState] = useState<string | undefined>(undefined);
  const [ready, setReady] = useState(false);

  const setQuery = (value: string) => {
    router.replace(
      {
        pathname: router.pathname,
        query: { ...router.query, [queryString]: value },
      },
      undefined,
      { shallow: true }
    );
  };

  useEffect(() => {
    // check if it's an array
    const val = router.query[queryString];
    if (!val) {
      setState(undefined);
    } else if (Array.isArray(val)) {
      setState(val[0]);
    } else {
      setState(val);
    }
  }, [router.query, queryString]);

  useEffect(() => {
    setReady(router.isReady);
  }, [router.isReady]);

  return {
    isReady: ready,
    query: state,
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
