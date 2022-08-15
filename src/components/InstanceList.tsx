import { Key, useEffect, useState } from 'react';
import InstanceCard from 'components/InstanceCard';
import { useFetch } from 'usehooks-ts';
import { useAppDispatch, useAppSelector } from 'utils/hooks';
import { selectInstanceList } from 'data/InstanceListSlice';

export default function InstanceList() {
  const { instances } = useAppSelector(selectInstanceList);
  const dispatch = useAppDispatch();

  console.log(instances);

  return (
    <div className="flex flex-col w-full grow">
      <h1 className="font-bold text-medium">Server Instances</h1>
      <div className="h-1 overflow-y-auto grow">
        {/* {instances &&
          instances.map((instance: { id: Key | null | undefined; title: string; }) => (
            <InstanceCard key={instance.id} name={instance.title} />
          ))} */}
      </div>
    </div>
  );
}
