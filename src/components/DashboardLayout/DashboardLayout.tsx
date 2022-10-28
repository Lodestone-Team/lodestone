// A react component that renders the left and top navbar for the dashboard.
// Also provides the instance context

import LeftNav from './LeftNav';
import TopNav from './TopNav';
import Split from 'react-split';
import { useWindowSize } from 'usehooks-ts';
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
  // called for side effects
  useEventStream();
  useClientInfo();
  const { width: windowWidth } = useWindowSize();
  const minWidth = (windowWidth / 12) * 1.5;
  const maxWidth = (windowWidth / 12) * 4;

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
          sizes={[10, 90]}
          minSize={[minWidth, 0]}
          maxSize={[maxWidth, Infinity]}
          snapOffset={0}
          gutterSize={0}
          className="flex flex-row items-stretch w-screen min-h-0 text-gray-300 bg-gray-800 grow"
        >
          <LeftNav />
          <div>{children}</div>
        </Split>
      </div>
    </InstanceContext.Provider>
  );
}
