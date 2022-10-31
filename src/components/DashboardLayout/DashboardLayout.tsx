// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import Split from 'react-split';
import { useInterval, useLocalStorage, useWindowSize } from 'usehooks-ts';
import { useEventStream } from 'data/EventStream';
import { useClientInfo } from 'data/SystemInfo';
import { InstanceContext } from 'data/InstanceContext';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useEffect, useState } from 'react';
import { useInstanceList } from 'data/InstanceList';
import { useRouterQuery } from 'utils/hooks';
import router from 'next/router';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { query: uuid } = useRouterQuery('uuid');
  const { isLoading, isError, data: instances, error } = useInstanceList();
  const [instance, setInstanceState] = useState<InstanceInfo | null>(null);
  const [splitSizes, setSplitSizes] = useLocalStorage(
    'dashboardSplitSize',
    [12.5, 87.5]
  );

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
        <Split
          sizes={splitSizes}
          minSize={[200, 0]}
          maxSize={[500, Infinity]}
          snapOffset={0}
          gutterSize={0}
          className="flex flex-row items-stretch w-screen min-h-0 text-gray-300 bg-gray-800 grow"
          onDragEnd={setSplitSizes}
        >
          <LeftNav />
          <div>{children}</div>
        </Split>
      </div>
    </InstanceContext.Provider>
  );
}
