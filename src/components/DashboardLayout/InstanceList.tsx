import InstanceCard from 'components/InstanceCard';
import { useRouter } from 'next/router';
import { useInstanceList } from 'data/InstanceList';

export default function InstanceList() {
  const { isLoading, isError, data: instances, error } = useInstanceList();
  const router = useRouter();

  // TODO: nicer looking loading and error indicators
  if (isLoading) {
    return <div>Loading...</div>;
  }
  if (isError) {
    return <div>Error: {error}</div>;
  }
  if (Object.keys(instances).length === 0) {
    return <div>No instances found</div>;
  }

  return (
    <div className="flex flex-col px-1 pt-1 -mx-1 overflow-y-auto h-fit gap-y-4 gap grow child:w-full">
      {instances &&
        Object.values(instances).map((instance) => (
          <InstanceCard
            key={instance.uuid}
            focus={router.query.uuid === instance.uuid}
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
