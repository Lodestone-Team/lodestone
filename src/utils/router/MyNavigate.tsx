import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { useContext, useEffect } from 'react';

export default function MyNavigate({ to }: { to: string }) {
  const { setPathname, location } = useContext(BrowserLocationContext);
  useEffect(() => {
    setPathname(to, true);
  }, [to, location]);
  return null;
}
