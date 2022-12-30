import React from 'react';
// eslint-disable-next-line no-restricted-imports -- this is the only place that should import useNavigate
import { useLocation, useNavigate, Location } from 'react-router-dom';

interface BrowserLocationContext {
  location: Location;
  searchParams: URLSearchParams;
  setLocation: (func: (loc: Location) => Location, replace?: boolean) => void;
  setPathname: (pathname: string, replace?: boolean) => void;
  setSearchParam: (key: string, value: string | undefined) => void;
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
 * Children can use this context to update the URL, but should use regular react-router-dom hooks
 * to read the URL.
 */
export const BrowserLocationContextProvider = ({
  children,
}: {
  children: React.ReactNode;
}) => {
  // location is let since we want to manually update it
  let location = useLocation();
  const navigate = useNavigate();
  const setLocation = (func: (loc: Location) => Location, replace = false) => {
    const newLocation = func(location);
    console.log('setLocation', {...location}, {...newLocation});
    location = newLocation;
    navigate(location, { replace });
  };
  const setPathname = (pathname: string, replace = false) => {
    console.log('setPathname', pathname);
    setLocation((loc) => ({ ...loc, pathname }), replace);
  };
  const setSearchParam = (key: string, value: string | undefined) => {
    console.log('setSearchParam', key, value);
    setLocation((loc) => {
      const newSearch = new URLSearchParams(loc.search);
      if (value === undefined) {
        newSearch.delete(key);
      } else {
        newSearch.set(key, value);
      }
      return { ...loc, search: newSearch.toString() };
    });
  };
  const navigateBack = () => {
    console.log('navigateBack');
    navigate(-1);
  };

  // searchParam is const since we don't use it for updating the URL
  // it's only provided in the context for convenience
  const searchParams = new URLSearchParams(location.search);

  const contextValue = {
    location,
    searchParams,
    setLocation,
    setPathname,
    setSearchParam,
    navigateBack,
  };

  return (
    <BrowserLocationContext.Provider value={contextValue}>
      {children}
    </BrowserLocationContext.Provider>
  );
};
