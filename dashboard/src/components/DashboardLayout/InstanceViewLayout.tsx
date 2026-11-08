import { InstanceInfo } from 'bindings/InstanceInfo';
import ResizePanel from 'components/Atoms/ResizePanel';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { InstanceContext, ModpackPreset } from 'data/InstanceContext';
import { useInstanceList } from 'data/InstanceList';
import { useUserLoggedIn } from 'data/UserInfo';
import { Suspense, useContext, useEffect, useState } from 'react';
import { Outlet } from 'react-router';
import { useLocalStorage } from 'usehooks-ts';
import { useQueryParam } from 'utils/hooks';
import LeftNav from './LeftNav';
import { PageLoading } from './LayoutSkeletons';

export const InstanceViewLayout = () => {
  const { setPathname } = useContext(BrowserLocationContext);
  const userLoggedIn = useUserLoggedIn();
  const [leftNavSize, setLeftNavSize] = useLocalStorage('leftNavSize', 220);
  /* Start Instances */
  const [queryInstanceId, setQueryInstanceId] = useQueryParam('instance', '');
  const { data: dataInstances, isFetched: instanceIsFetched } =
    useInstanceList(userLoggedIn);
  const [showCreateInstance, setShowCreateInstance] = useState(false);
  const [modpackPreset, setModpackPreset] = useState<ModpackPreset | null>(
    null
  );

  const instances = userLoggedIn ? dataInstances : undefined;
  // Derived, not copied into state by an effect: the copy was a render behind
  // the URL and the list, so every selection (and every page load) first
  // rendered with no instance, flashing the loading or empty state.
  const instance: InstanceInfo | null =
    (queryInstanceId && instances?.[queryInstanceId]) || null;

  useEffect(() => {
    // While the list loads, keep the URL: a link to an instance's tab (for
    // example /dashboard/mods?instance=…&modrinth=…) must not be sent home.
    if (queryInstanceId && !instances) return;
    // the settings modal is open over this page: its URL is a settings page
    if (location.pathname.startsWith('/settings/')) return;
    if (queryInstanceId && instances && queryInstanceId in instances) {
      if (!location.pathname.startsWith('/dashboard'))
        setPathname('/dashboard/overview');
    } else {
      if (location.pathname.startsWith('/dashboard')) setPathname('/');
    }
  }, [instances, queryInstanceId]);

  function selectInstance(instance: InstanceInfo | null) {
    if (instance === null) {
      setQueryInstanceId('');
      if (location.pathname.startsWith('/dashboard')) setPathname('/');
    } else {
      setQueryInstanceId(instance.uuid);
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
        showCreateInstance: showCreateInstance,
        setShowCreateInstance: setShowCreateInstance,
        modpackPreset,
        setModpackPreset,
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
        <LeftNav className="border-r border-fade-700 bg-gray-850 select-none" />
      </ResizePanel>
      <div className="mx-8 h-full min-w-0 grow child:h-full">
        <Suspense fallback={<PageLoading />}>
          <Outlet />
        </Suspense>
      </div>
    </InstanceContext.Provider>
  );
};
