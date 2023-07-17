import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import MyNavigate from './MyNavigate';

export default function RequireCore({
  children,
  redirect = '/first_setup',
}: {
  children: React.ReactNode;
  redirect?: string;
}) {
  const { coreList } = useContext(LodestoneContext);
  return coreList.length > 0 ? <>{children}</> : <MyNavigate to={redirect} />;
}
