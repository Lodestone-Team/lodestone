import { Key, useEffect, useState } from 'react';
import InstanceCard from 'components/InstanceCard';
import { useAppDispatch, useAppSelector } from 'utils/hooks';
import { fetchInstanceList, selectInstanceList } from 'data/InstanceList';
import { selectClientInfo } from 'data/ClientInfo';
import { useRouter } from 'next/router';

export default function InstanceList() {
  const { instances, loading, error } = useAppSelector(selectInstanceList);
  const dispatch = useAppDispatch();
  const clientInfo = useAppSelector(selectClientInfo);
  const router = useRouter();

  useEffect(() => {
    if (clientInfo.loading) return;
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
    <div className="flex flex-col px-1 pt-1 -mx-1 overflow-y-auto h-fit gap-y-4 gap grow child:w-full">
      {instances &&
        Object.values(instances).map((instance) => (
          <InstanceCard
            key={instance.id}
            focus={router.query.uuid === instance.id}
            onClick={() => {
              // redirect to /dashboard and add the instance id to the query string
              router.push(
                {
                  pathname: '/dashboard',
                  query: {
                    ...router.query,
                    uuid: instance.id,
                  },
                },
                undefined,
                { shallow: true }
              );
            }}
            {...instance}
          />
        ))}
    </div>
  );
}
