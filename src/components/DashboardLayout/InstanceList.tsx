import InstanceCard from 'components/InstanceCard';
import { useInstanceList } from 'data/InstanceList';
import router from 'next/router';
import { useRouterQuery } from 'utils/hooks';

export default function InstanceList() {
  const { isLoading, isError, data: instances, error } = useInstanceList();
  const {query: uuid} = useRouterQuery('uuid');

  // TODO: nicer looking loading and error indicators
  if (isLoading) {
    return <div>Loading...</div>;
  }
  if (isError) {
    return <div>Error: {error.message}</div>;
  }
  if (Object.keys(instances).length === 0) {
    return <div>No instances found</div>;
  }

  return (
    <div className="flex flex-col px-1.5 pt-1.5 -mx-1.5 overflow-y-auto gap-y-4 gap grow child:w-full h-0 pb-3">
      {instances &&
        Object.values(instances).map((instance) => (
          <InstanceCard
            key={instance.uuid}
            focus={uuid === instance.uuid}
            onClick={() => {
              // redirect to /dashboard and add the instance id to the query string
              router.push(
                {
                  pathname: '/dashboard',
                  query: {
                    ...router.query,
                    uuid: instance.uuid,
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
