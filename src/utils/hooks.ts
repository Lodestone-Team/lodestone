import { BrowserLocationContext } from './../data/BrowserLocationContext';
import { useCallback, useEffect, useRef, useState, useContext } from 'react';
import { useIsomorphicLayoutEffect, useLocalStorage } from 'usehooks-ts';
import ReactGA from 'react-ga4';

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

export let globalSearchParams: URLSearchParams;

/**
 * Similar to useState, but stores the value in the search params.
 * @param key The key to use in the search params.
 * @param initialValue The initial value to use and the default value if the key is not present.
 * @param visible If the default value should be visible in the search params.
 * @return A tuple of the value and a setter function.
 */
export function useQueryParam(
  key: string,
  initialValue: string,
  visible = true
) {
  const { setSearchParam, searchParams } = useContext(BrowserLocationContext);
  const [value, setValue] = useState(searchParams.get(key) ?? initialValue);

  const setValueAndParams = useCallback(
    (newValue: string) => {
      setValue(newValue);
      setSearchParam(key, newValue, true);
    },
    [key, setSearchParam]
  );

  useEffect(() => {
    const newValue = searchParams.get(key);
    // if value is falsy and initial value is not the same (to prevent infinite loops), set the param to the initial value
    if (!newValue && newValue !== initialValue) {
      if (visible)
        //if visible, set the searchParam too
        setSearchParam(key, initialValue, true);
      // if not visible, just set the value
      else setValue(initialValue);
    } else if (!newValue) {
      // always set the internal value anyways
      setValue(initialValue);
    }
    // if value is truthy and not the same as the current value, set the value
    if (newValue && newValue !== value) {
      setValue(newValue);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialValue, key, searchParams]);

  return [value, setValueAndParams] as const;
}

/**
 * Similar to useState, but stores the value in both local storage and the search params.
 * @param key The key to use in the search params and local storage.
 * @param initialValue The initial value to use.
 * @param visible If the stored value should be visible in the search params.
 * @return A tuple of the value and a setter function.
 */
export function useLocalStorageQueryParam(
  key: string,
  initialValue: string,
  visible = true
) {
  const { setSearchParam, searchParams } = useContext(BrowserLocationContext);
  const [value, setValue] = useLocalStorage(key, initialValue);

  const setValueAndParams = useCallback(
    (newValue: string) => {
      setValue(newValue);
      // if empty, remove the param
      setSearchParam(key, newValue, true);
    },
    [key, setSearchParam, setValue]
  );

  // we use the stored value as the initial value
  useEffect(() => {
    const newValue = searchParams.get(key);
    // if value is falsy and not the same as the current value, set the param to the current value if visible
    if (!newValue && newValue !== value) {
      if (visible)
        //if visible, set the searchParam too
        setSearchParam(key, value, true);
    }
    // if value is truthy and not the same as the current value, set the value
    if (newValue && newValue !== value) {
      setValue(newValue);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialValue, key, searchParams]);

  return [value, setValueAndParams] as const;
}

export function usePrevious(value: unknown) {
  const ref = useRef(value);
  useEffect(() => {
    ref.current = value;
  });
  return ref.current;
}

export const useAnalyticsEventTracker = (category: string) => {
  const eventTracker = (action = 'test action', label?: string) => {
    ReactGA.event({ category, action, label });
  };
  return eventTracker;
};
export default useAnalyticsEventTracker;
