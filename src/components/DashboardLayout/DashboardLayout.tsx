import LeftNav from './LeftNav';
import TopNav from './TopNav';
import { useContext } from 'react';
import { useLocalStorage, useWindowSize } from 'usehooks-ts';
import { useEventStream } from 'data/EventStream';
import { useCoreInfo, useLocalCoreInfo } from 'data/SystemInfo';
import { InstanceContext } from 'data/InstanceContext';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useEffect, useState } from 'react';
import { useInstanceList } from 'data/InstanceList';
import { useQueryParam } from 'utils/hooks';
import ResizePanel from 'components/Atoms/ResizePanel';
import NotificationPanel from './NotificationPanel';
import { useUserLoggedIn } from 'data/UserInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { Outlet } from 'react-router-dom';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { DEFAULT_LOCAL_CORE, LODESTONE_PORT } from 'utils/util';
import { LodestoneContext } from 'data/LodestoneContext';

export default function DashboardLayout() {
  const { setPathname, location } = useContext(BrowserLocationContext);
  const userLoggedIn = useUserLoggedIn();
  useEventStream();

  /* Start Notification */
  const [rightNavSize, setRightNavSize] = useLocalStorage('rightNavSize', 200);
  const [showNotifications, setShowNotifications] = useLocalStorage(
    'showNotifications',
    false
  );
  const { width } = useWindowSize();
  /* End Notification */

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

  /* Start Core */
  const { setCore, addCore, coreConnectionStatus, core } =
    useContext(LodestoneContext);
  const [showSetupPrompt, setShowSetupPrompt] = useState(false);
  const [showLocalSetupPrompt, setShowLocalSetupPrompt] = useState(false);
  const { data: coreInfo } = useCoreInfo();
  const { data: localCoreInfo } = useLocalCoreInfo();
  useEffect(() => {
    if (coreInfo?.is_setup === false) {
      setShowSetupPrompt(true);
    }
  }, [coreInfo]);

  useEffect(() => {
    if (localCoreInfo?.is_setup === false) {
      if (!showSetupPrompt) setShowLocalSetupPrompt(true);
    } else if (localCoreInfo?.is_setup === true) {
      addCore(DEFAULT_LOCAL_CORE);
    }
  }, [localCoreInfo, showSetupPrompt]);
  /* End Core */

  return (
    <InstanceContext.Provider
      value={{
        instanceList: instances || {},
        selectedInstance: instance,
        selectInstance: setInstance,
      }}
    >
      <ConfirmDialog
        isOpen={showLocalSetupPrompt}
        title="New Local Core Detected"
        type="info"
        confirmButtonText="Setup"
        onConfirm={() => {
          setCore(DEFAULT_LOCAL_CORE);
          setPathname('/login/core/first_setup');
          setShowLocalSetupPrompt(false);
        }}
        closeButtonText="Skip"
        onClose={() => {
          setShowLocalSetupPrompt(false);
        }}
      >
        Detected a local core that is not setup yet. Would you like to setup{' '}
        {localCoreInfo?.core_name}?
      </ConfirmDialog>
      <ConfirmDialog
        isOpen={showSetupPrompt}
        title="Setup Required"
        type="info"
        z-index="20"
        confirmButtonText="Setup"
        onConfirm={() => {
          setPathname('/login/core/first_setup');
          setShowSetupPrompt(false);
        }}
        closeButtonText="Change Core"
        onClose={() => {
          setPathname('/login/core/select');
          setShowSetupPrompt(false);
        }}
      >
        {coreInfo?.core_name} is not setup yet. Please complete the setup
        process.
      </ConfirmDialog>
      <ConfirmDialog
        isOpen={coreConnectionStatus === 'error'}
        title="Core Connection Error"
        type="info"
        z-index="20"
        confirmButtonText="Change Core"
        onConfirm={() => {
          setPathname('/login/core/select');
        }}
        closeButtonText="Refresh"
        onClose={() => {
          window.location.reload();
        }}
      >
        There was an error connecting to {core.address}:{core.port}. Please select a different
        core, refresh the page, or simply wait for the core to come back online.
      </ConfirmDialog>
      <div className="flex h-screen flex-col">
        <TopNav
          showNotifications={showNotifications}
          setShowNotifications={setShowNotifications}
        />
        <div className="relative flex min-h-0 w-full grow flex-row bg-gray-850">
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
          {showNotifications &&
            (width > 1280 ? (
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
            ) : (
              <div
                className="absolute right-2 -top-2 h-full w-96 rounded-lg drop-shadow-lg child:h-full"
                style={{
                  width: rightNavSize,
                }}
              >
                <NotificationPanel className="rounded-lg border" />
              </div>
            ))}
        </div>
      </div>
    </InstanceContext.Provider>
  );
}
