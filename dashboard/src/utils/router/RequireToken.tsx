import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import MyNavigate from './MyNavigate';

export default function RequireToken({
  children,
  redirect = '/login/user/select',
}: {
  children: React.ReactNode;
  redirect?: string;
}) {
  const { token } = useContext(LodestoneContext);
  return token ? <>{children}</> : <MyNavigate to={redirect} />;
}
