import React, { useCallback, useMemo } from 'react';
// eslint-disable-next-line no-restricted-imports -- this is the only place that should import useNavigate
import { useLocation, useNavigate, Location } from 'react-router';

interface BrowserLocationContext {
  location: Location;
  searchParams: URLSearchParams;
  setLocation: (func: (loc: Location) => Location, replace?: boolean) => void;
  setPathname: (pathname: string, replace?: boolean) => void;
  setSearchParam: (
    key: string,
    value: string | undefined,
    replace?: boolean
  ) => void;
  navigateBack: () => void;
}

export const BrowserLocationContext =
  React.createContext<BrowserLocationContext>({
    location: {
      pathname: '',
      search: '',
      hash: '',
      state: undefined,
      key: '',
    },
    searchParams: new URLSearchParams(),
    setLocation: () => {
      throw new Error('Not implemented');
    },
    setPathname: () => {
      throw new Error('Not implemented');
    },
    setSearchParam: () => {
      throw new Error('Not implemented');
    },
    navigateBack: () => {
      throw new Error('Not implemented');
    },
  });

/**
 * We store too many information in the URL, and we update them too often.
 * There acts as the single source of truth for the URL (location) and provides
 * a way to update the URL.
 * Children can use this context to update the URL, but should use regular react-router hooks
 * to read the URL.
 */
export const BrowserLocationContextProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  const location = useLocation();
  const navigate = useNavigate();

  // Setters start from the browser's current URL rather than the location of
  // the last render, so several setters called in a row build on each other
  // (BrowserRouter updates window.location synchronously on navigate).
  const setLocation = useCallback(
    (func: (loc: Location) => Location, replace = false) => {
      const current: Location = {
        pathname: window.location.pathname,
        search: window.location.search,
        hash: window.location.hash,
        state: null,
        key: 'default',
      };
      navigate(func(current), { replace });
    },
    [navigate]
  );
  const setPathname = useCallback(
    (pathname: string, replace = false) => {
      if (pathname === window.location.pathname) return;
      setLocation((loc) => ({ ...loc, pathname }), replace);
    },
    [setLocation]
  );
  const setSearchParam = useCallback(
    (key: string, value: string | undefined, replace = false) => {
      setLocation((loc) => {
        const newSearch = new URLSearchParams(loc.search);
        if (value === undefined) {
          newSearch.delete(key);
        } else {
          newSearch.set(key, value);
        }
        return { ...loc, search: newSearch.toString() };
      }, replace);
    },
    [setLocation]
  );
  const navigateBack = useCallback(() => {
    navigate(-1);
  }, [navigate]);

  // searchParam is only provided in the context for convenience, it is not
  // used for updating the URL
  const searchParams = useMemo(
    () => new URLSearchParams(location.search),
    [location.search]
  );

  const contextValue = useMemo(
    () => ({
      location,
      searchParams,
      setLocation,
      setPathname,
      setSearchParam,
      navigateBack,
    }),
    [
      location,
      searchParams,
      setLocation,
      setPathname,
      setSearchParam,
      navigateBack,
    ]
  );

  return (
    <BrowserLocationContext.Provider value={contextValue}>
      {children}
    </BrowserLocationContext.Provider>
  );
};
