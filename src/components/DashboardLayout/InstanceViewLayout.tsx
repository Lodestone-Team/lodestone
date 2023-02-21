import { InstanceInfo } from 'bindings/InstanceInfo';
import ResizePanel from 'components/Atoms/ResizePanel';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceList } from 'data/InstanceList';
import { useUserLoggedIn } from 'data/UserInfo';
import { useContext, useEffect, useState } from 'react';
import { Outlet } from 'react-router-dom';
import { useLocalStorage } from 'usehooks-ts';
import { useQueryParam } from 'utils/hooks';
import LeftNav from './LeftNav';

export const InstanceViewLayout = () => {
  const { setPathname } = useContext(BrowserLocationContext);
  const userLoggedIn = useUserLoggedIn();
  const [leftNavSize, setLeftNavSize] = useLocalStorage('leftNavSize', 220);
  /* Start Instances */
  const [queryInstanceId, setQueryInstanceId] = useQueryParam('instance', '');
  const { data: dataInstances, isFetched: instanceIsFetched } =
    useInstanceList(userLoggedIn);
  const [instance, setInstanceState] = useState<InstanceInfo | undefined>(
    undefined
  );
  const instances = userLoggedIn ? dataInstances : undefined;

  useEffect(() => {
    console.log(queryInstanceId);
    if (queryInstanceId && instances && queryInstanceId in instances) {
      setInstanceState(instances[queryInstanceId]);
      if (!location.pathname.startsWith('/dashboard'))
        setPathname('/dashboard/console');
    } else {
      setInstanceState(undefined);
      if (location.pathname.startsWith('/dashboard')) setPathname('/');
    }
  }, [instances, queryInstanceId]);

  function selectInstance(instance?: InstanceInfo) {
    console.log('selectInstance', instance);
    if (instance === undefined) {
      setInstanceState(undefined);
      setQueryInstanceId('');
      if (location.pathname.startsWith('/dashboard')) setPathname('/');
    } else {
      setInstanceState(instance);
      setQueryInstanceId(instance.uuid);
      setPathname('/dashboard/console');
    }
  }
  /* End Instances */

  return (
    <InstanceContext.Provider
      value={{
        instanceList: instances || {},
        selectedInstance: instance,
        selectInstance: selectInstance,
        isReady: instanceIsFetched && userLoggedIn,
      }}
    >
      <ResizePanel
        direction="e"
        maxSize={280}
        minSize={200}
        size={leftNavSize}
        validateSize={false}
        onResize={setLeftNavSize}
        containerClassNames="min-h-0"
      >
        <LeftNav className="select-none border-r border-fade-700 bg-gray-850" />
      </ResizePanel>
      <div className="mx-8 h-full min-w-0 grow child:h-full">
        <Outlet />
      </div>
    </InstanceContext.Provider>
  );
};
