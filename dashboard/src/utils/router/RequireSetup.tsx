import { useCoreInfo } from 'data/SystemInfo';
import { useEffect, useState } from 'react';
import MyNavigate from './MyNavigate';

// how long to wait for the core info before showing the page anyway
const LENIENT_WAIT_MS = 1000;

export default function RequireSetup({
  children,
  redirect = '/login/core/first_setup',
  lenient = true, // if true, don't wait for a slow or failing fetch
}: {
  children: React.ReactNode;
  redirect?: string;
  lenient?: boolean;
}) {
  const { data: coreInfo, isFetched } = useCoreInfo();
  const { is_setup } = coreInfo ?? {};
  // While the core info loads, show nothing rather than the page: a core that
  // isn't set up would otherwise flash the sign-in page before the redirect.
  // A lenient check stops waiting after a moment (the core may be slow, or
  // retrying a failed request) and shows the page, as it always did.
  const [waitedLong, setWaitedLong] = useState(false);
  useEffect(() => {
    if (!lenient || isFetched) return;
    const timer = setTimeout(() => setWaitedLong(true), LENIENT_WAIT_MS);
    return () => clearTimeout(timer);
  }, [lenient, isFetched]);

  if (!isFetched) {
    if (!lenient) return <MyNavigate to={redirect} />;
    return waitedLong ? <>{children}</> : null;
  }
  return !is_setup ? <MyNavigate to={redirect} /> : <>{children}</>;
}
