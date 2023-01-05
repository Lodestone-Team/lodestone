import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { useContext, useEffect } from 'react';

export default function MyNavigate({ to }: { to: string }) {
  const { setPathname, location } = useContext(BrowserLocationContext);
  useEffect(() => {
    setPathname(to, true);
  }); //no dependencies so it runs repeatedly to gurantee the redirect
  return null;
}
