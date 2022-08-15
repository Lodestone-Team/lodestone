import { useEffect, useState } from 'react';
import InstanceCard from './InstanceCard';

interface placeholderInstanceType {
  userId: string;
  id: string;
  title: string;
  completed: boolean;
}

export default function InstanceList() {
  const [instances, setInstances] = useState<placeholderInstanceType[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch('https://jsonplaceholder.typicode.com/todos')
      .then((res) => res.json())
      .then((data) => {
        setInstances(data.slice(0, 4));
        setLoading(false);
      })
      .catch((err) => {
        console.log(err);
        setLoading(false);
      });
  }, []);

  return (
    <div className="flex flex-col w-full grow">
      <h1 className="font-bold text-medium">Server Instances</h1>
      <div className="h-1 overflow-y-auto grow">
        {instances.map((instance) => (
          <InstanceCard key={instance.id} name={instance.title} />
        ))}
      </div>
    </div>
  );
}
