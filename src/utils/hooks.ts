import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { useRouter } from 'next/router';
import { useEffect, useRef, useState, useLayoutEffect } from 'react';
import { useCookies } from 'react-cookie';

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

/**
 * setToken is a function that sets the token in the cookie and in the axios header, and then refetches the queryClient
 * removeToken is a function that removes the token from the cookie and from the axios header, and then refetches the queryClient
 */
export function useToken() {
  const [cookies, setCookie, removeCookie] = useCookies(['token']);
  const [token, setToken] = useState<string | undefined>(undefined);
  const queryClient = useQueryClient();

  // We use a useLayoutEffect here instead of cookies directly to avoid this "first render" difference between server and client
  useLayoutEffect(() => {
    setToken(cookies.token);
  }, [cookies.token]);

  return {
    token,
    setToken: (token: string) => {
      setCookie('token', token);
      axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
      queryClient.invalidateQueries();
    },
    removeToken: () => {
      removeCookie('token');
      delete axios.defaults.headers.common['Authorization'];
      queryClient.invalidateQueries();
    },
  };
}
