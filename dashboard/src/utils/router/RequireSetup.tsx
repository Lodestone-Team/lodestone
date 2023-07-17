import { useCoreInfo } from 'data/SystemInfo';
import MyNavigate from './MyNavigate';

export default function RequireSetup({
  children,
  redirect = '/login/core/first_setup',
  lenient = true, // if true, don't redirect if failed to fetch
}: {
  children: React.ReactNode;
  redirect?: string;
  lenient?: boolean;
}) {
  const { data: coreInfo, isFetched } = useCoreInfo();
  const { is_setup } = coreInfo ?? {};

  const shouldRedirect = !isFetched ? !lenient : !is_setup;

  return shouldRedirect ? <MyNavigate to={redirect} /> : <>{children}</>;
}
