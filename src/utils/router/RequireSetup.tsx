import { LodestoneContext } from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';
import { useContext } from 'react';
import MyNavigate from './MyNavigate';

export default function RequireSetup({
  children,
  redirect = '/login/user/select',
}: {
  children: React.ReactNode;
  redirect?: string;
}) {
  const { data: coreInfo } = useCoreInfo();
  const { is_setup } = coreInfo ?? {};

  return is_setup ? <>{children}</> : <MyNavigate to={redirect} />;
}
