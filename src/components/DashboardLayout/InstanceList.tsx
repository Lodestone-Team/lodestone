import InstanceCard from 'components/InstanceCard';
import InstanceLoadingCard from 'components/InstanceLoadingCard';
import { InstanceContext } from 'data/InstanceContext';
import { NotificationContext } from 'data/NotificationContext';
import { useContext } from 'react';
import { match, otherwise } from 'variant';

export default function InstanceList({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  const {
    instanceList: instances,
    selectedInstance,
    selectInstance,
  } = useContext(InstanceContext);
  const { ongoingNotifications } = useContext(NotificationContext);

  const InstanceCreations = ongoingNotifications.filter((notification) => {
    return (
      notification.start_value &&
      notification.start_value.type === 'InstanceCreation'
    );
  });

  return (
    <div
      className={`gap -mx-1.5 flex h-0 grow flex-col gap-y-4 overflow-y-auto px-3 child:w-full ${className}`}
    >
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
      {ongoingNotifications &&
        ongoingNotifications
          .map((notification) => {
            if (!notification.start_value) return null;
            if (notification.state === 'done') return null;
            return match(
              notification.start_value,
              otherwise(
                {
                  InstanceCreation: ({
                    instance_uuid,
                    instance_name,
                    port,
                    flavour,
                    game_type,
                  }) => (
                    <InstanceLoadingCard
                      key={instance_uuid}
                      uuid={instance_uuid}
                      name={instance_name}
                      port={port}
                      flavour={flavour}
                      game_type={game_type}
                      level={notification.level}
                      state={notification.state}
                      progress_percent={
                        notification.total
                          ? notification.progress / notification.total
                          : undefined
                      }
                      progress_title={'Setting up...'}
                    />
                  ),
                },
                (_) => null
              )
            );
          })
          .reverse()}
      {children}
    </div>
  );
}
