import { BrowserLocationContext } from './../data/BrowserLocationContext';
import {
  EffectCallback,
  useCallback,
  useEffect,
  useRef,
  useState,
  useContext,
} from 'react';
import { useIsomorphicLayoutEffect, useLocalStorage } from 'usehooks-ts';
import { hashKey, QueryKey, useQueryClient } from '@tanstack/react-query';
import ReactGA from 'react-ga4';

/**
 * Runs an effect only on mount (and its cleanup on unmount).
 * Replaces useEffectOnce, which was removed in usehooks-ts 3.
 */
export function useEffectOnce(effect: EffectCallback) {
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(effect, []);
}

/**
 * Calls onSuccess / onError every time the query with this key finishes a
 * fetch while the calling component is mounted, like the useQuery callbacks
 * that were removed in react-query 5. Manual setQueryData updates do not call
 * onSuccess, same as before.
 */
export function useQueryFetchCallbacks<TData = unknown, TError = Error>(
  queryKey: QueryKey,
  {
    onSuccess,
    onError,
  }: {
    onSuccess?: (data: TData) => void;
    onError?: (error: TError) => void;
  }
) {
  const queryClient = useQueryClient();
  const queryHash = hashKey(queryKey);
  const onSuccessRef = useRef(onSuccess);
  const onErrorRef = useRef(onError);

  // Remember the latest callbacks if they change.
  useIsomorphicLayoutEffect(() => {
    onSuccessRef.current = onSuccess;
    onErrorRef.current = onError;
  });

  useEffect(
    () =>
      queryClient.getQueryCache().subscribe((event) => {
        if (event.type !== 'updated' || event.query.queryHash !== queryHash)
          return;
        const { action } = event;
        if (action.type === 'success' && !action.manual) {
          onSuccessRef.current?.(action.data as TData);
        } else if (action.type === 'error') {
          onErrorRef.current?.(action.error as TError);
        }
      }),
    [queryClient, queryHash]
  );
}

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
    (newValue: string, replace = true) => {
      setValue(newValue);
      setSearchParam(key, newValue, replace);
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
  visible = true,
) {
  const { setSearchParam, searchParams } = useContext(BrowserLocationContext);
  const [value, setValue] = useLocalStorage(key, initialValue);

  const setValueAndParams = useCallback(
    (newValue: string, replace = true) => {
      setValue(newValue);
      // if empty, remove the param
      setSearchParam(key, newValue, replace);
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
  const eventTracker = (
    action = 'test action',
    label?: string,
    nonInteraction?: boolean,
    value?: number
  ) => {
    ReactGA.event({ category, action, label, nonInteraction, value });
  };
  return eventTracker;
};
export default useAnalyticsEventTracker;
