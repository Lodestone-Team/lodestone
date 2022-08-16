import { Key, useEffect, useState } from 'react';
import InstanceCard from './InstanceCard';
import { useFetch } from 'usehooks-ts';
import { useAppDispatch, useAppSelector } from 'utils/hooks';
import { fetchInstanceList, selectInstanceList } from 'data/InstanceList';
import { selectClientInfo } from 'data/ClientInfo';

export default function InstanceList() {
  const { instances, loading, error } = useAppSelector(selectInstanceList);
  const dispatch = useAppDispatch();
  const clientInfo = useAppSelector(selectClientInfo);

  useEffect(() => {
    if(clientInfo.loading)
      return;
    dispatch(fetchInstanceList(clientInfo));
  }, [dispatch, clientInfo]);

  // TODO: nicer looking loading and error indicators
  if (loading) {
    return <div>Loading...</div>;
  }
  if (error) {
    return <div>Error: {error}</div>;
  }
  if (!instances) {
    return <div>No instances found</div>;
  }

  return (
    <div className="h-1 overflow-y-auto grow">
      {instances &&
        Object.values(instances).map((instance) => (
          <InstanceCard key={instance.id} name={instance.name} />
        ))}
    </div>
  );
}
