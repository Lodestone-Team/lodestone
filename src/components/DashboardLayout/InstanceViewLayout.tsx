import { InstanceInfo } from 'bindings/InstanceInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceList } from 'data/InstanceList';
import { useUserLoggedIn } from 'data/UserInfo';
import { useContext, useEffect, useState } from 'react';
import { Outlet } from 'react-router-dom';
import { useQueryParam } from 'utils/hooks';
import LeftNav from './LeftNav';

export const InstanceViewLayout = () => {
  const { setPathname } = useContext(BrowserLocationContext);
  const userLoggedIn = useUserLoggedIn();
  /* Start Instances */
  const [queryUuid, setQueryUuid] = useQueryParam('instance', '');
  const { data: dataInstances } = useInstanceList(userLoggedIn);
  const [instance, setInstanceState] = useState<InstanceInfo | undefined>(
    undefined
  );
  const instances = userLoggedIn ? dataInstances : undefined;

  useEffect(() => {
    if (queryUuid && instances && queryUuid in instances)
      setInstanceState(instances[queryUuid]);
    else setInstanceState(undefined);
  }, [instances, queryUuid]);

  function setInstance(instance?: InstanceInfo) {
    console.log('setInstance', instance);
    if (instance === undefined) {
      setInstanceState(undefined);
      setQueryUuid('');
      if (location.pathname.startsWith('/dashboard')) setPathname('/');
    } else {
      setInstanceState(instance);
      setQueryUuid(instance.uuid);
      setPathname('/dashboard');
    }
  }
  /* End Instances */

  return (
    <InstanceContext.Provider
      value={{
        instanceList: instances || {},
        selectedInstance: instance,
        selectInstance: setInstance,
      }}
    >
      <div className="flex grow flex-row justify-center gap-[1vw]">
        <div className="flex h-full grow basis-60 flex-row flex-nowrap items-stretch justify-end">
          <div className="h-full w-[16rem] max-w-[16rem] child:h-full">
            <LeftNav />
          </div>
        </div>
        <div className="h-full min-w-0 grow basis-[1024px] child:h-full">
          <div className="max-w-[1024px]">
            <Outlet />
          </div>
        </div>
      </div>
    </InstanceContext.Provider>
  );
};
