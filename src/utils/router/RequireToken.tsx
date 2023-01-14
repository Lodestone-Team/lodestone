import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import MyNavigate from './MyNavigate';

export default function RequireToken({
  children,
  redirect = '/login/core/first_setup',
}: {
  children: React.ReactNode;
  redirect?: string;
}) {
  const { token } = useContext(LodestoneContext);
  return token ? <>{children}</> : <MyNavigate to={redirect} />;
}
