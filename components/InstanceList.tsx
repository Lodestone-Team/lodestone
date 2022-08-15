import { useEffect, useState } from 'react';
import InstanceCard from 'components/InstanceCard';
import { useFetch } from 'usehooks-ts';

interface placeholderInstanceType {
  userId: string;
  id: string;
  title: string;
  completed: boolean;
}

const placeHolderUrl = 'https://jsonplaceholder.typicode.com/todos';

export default function InstanceList() {
  const { data: instances, error } =
    useFetch<placeholderInstanceType[]>(placeHolderUrl);

  return (
    <div className="flex flex-col w-full grow">
      <h1 className="font-bold text-medium">Server Instances</h1>
      <div className="h-1 overflow-y-auto grow">
        {error && <div>Error: {error.message}</div>}
        {instances &&
          instances.map((instance) => (
            <InstanceCard key={instance.id} name={instance.title} />
          ))}
      </div>
    </div>
  );
}
