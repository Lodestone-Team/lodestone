import InstanceCard from 'components/InstanceCard';
import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';

export default function InstanceList() {
  const {
    instanceList: instances,
    selectedInstance,
    selectInstance,
  } = useContext(InstanceContext);

  return (
    <div className="gap -mx-1.5 flex h-0 grow flex-col gap-y-4 overflow-y-auto px-1.5 pt-1.5 pb-3 child:w-full">
      {instances &&
        Object.values(instances).map((instance) => (
          <InstanceCard
            key={instance.uuid}
            focus={selectedInstance?.uuid === instance.uuid}
            onClick={() => {
              selectInstance(instance);
            }}
            {...instance}
          />
        ))}
    </div>
  );
}
