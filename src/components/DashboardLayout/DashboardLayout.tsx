// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import { useInterval, useLocalStorage, useWindowSize } from 'usehooks-ts';
import { useEventStream } from 'data/EventStream';
import { useClientInfo } from 'data/SystemInfo';
import { InstanceContext } from 'data/InstanceContext';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useEffect, useState } from 'react';
import { useInstanceList } from 'data/InstanceList';
import { useRouterQuery } from 'utils/hooks';
import router from 'next/router';
import ResizePanel from 'components/Atoms/ResizePanel';
import NotificationPanel from './NotificationPanel';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { query: uuid } = useRouterQuery('uuid');
  const { isLoading, isError, data: instances, error } = useInstanceList();
  const [instance, setInstanceState] = useState<InstanceInfo | null>(null);
  const [leftNavSize, setLeftNavSize] = useLocalStorage('leftNavSize', 200);
  const [rightNavSize, setRightNavSize] = useLocalStorage('rightNavSize', 200);

  // called for side effects
  useEventStream();
  useClientInfo();

  useEffect(() => {
    if (uuid && instances && uuid in instances)
      setInstanceState(instances[uuid]);
    else setInstanceState(null);
  }, [instances, uuid]);

  function setInstance(instance: InstanceInfo | null) {
    if (instance === null) {
      setInstanceState(null);
      router.push(
        {
          pathname: '/',
          query: {
            ...router.query,
            uuid: null,
          },
        },
        undefined,
        { shallow: true }
      );
    } else {
      setInstanceState(instance);
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
    }
  }

  return (
    <InstanceContext.Provider
      value={{
        instanceList: instances || {},
        selectedInstance: instance,
        selectInstance: setInstance,
      }}
    >
      <div className="flex flex-col h-screen">
        <TopNav />
        <div className="flex flex-row w-full min-h-0 grow">
          <ResizePanel
            direction="e"
            maxSize={500}
            minSize={200}
            size={leftNavSize}
            validateSize={false}
            onResize={setLeftNavSize}
            containerClassNames="min-h-0"
          >
            <LeftNav />
          </ResizePanel>
          <div className="h-full min-w-0 grow child:h-full">{children}</div>
          <ResizePanel
            direction="w"
            maxSize={500}
            minSize={200}
            size={rightNavSize}
            validateSize={false}
            onResize={setRightNavSize}
            containerClassNames="min-h-0"
          >
            <NotificationPanel />
          </ResizePanel>
        </div>
      </div>
    </InstanceContext.Provider>
  );
}
