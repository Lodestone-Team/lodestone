import LeftNav from './LeftNav';
import TopNav from './TopNav';
import { useContext } from 'react';
import { useLocalStorage, useWindowSize } from 'usehooks-ts';
import { useEventStream } from 'data/EventStream';
import { useCoreInfo } from 'data/SystemInfo';
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

export default function DashboardLayout() {
  const { setPathname } = useContext(BrowserLocationContext);
  const [queryUuid, setQueryUuid] = useQueryParam('instance', '');
  const userLoggedIn = useUserLoggedIn();
  const { data: dataInstances } = useInstanceList(userLoggedIn);
  const [instance, setInstanceState] = useState<InstanceInfo | undefined>(
    undefined
  );
  const [rightNavSize, setRightNavSize] = useLocalStorage('rightNavSize', 200);
  const [showNotifications, setShowNotifications] = useLocalStorage(
    'showNotifications',
    false
  );
  const [showSetupPrompt, setShowSetupPrompt] = useState(false);
  const { width, height } = useWindowSize();

  const instances = userLoggedIn ? dataInstances : undefined;

  useEventStream();
  const { data: coreInfo } = useCoreInfo();
  useEffect(() => {
    if (coreInfo?.is_setup === false) {
      setShowSetupPrompt(true);
    }
  }, [coreInfo]);

  useEffect(() => {
    if (queryUuid && instances && queryUuid in instances)
      setInstanceState(instances[queryUuid]);
    else setInstanceState(undefined);
  }, [instances, queryUuid]);

  function setInstance(instance?: InstanceInfo) {
    if (instance === undefined) {
      setInstanceState(undefined);
      setQueryUuid('');
      setPathname('/');
    } else {
      setInstanceState(instance);
      setQueryUuid(instance.uuid);
      setPathname('/dashboard');
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
      <ConfirmDialog
        isOpen={showSetupPrompt}
        title="Setup Required"
        type="info"
        onClose={() => {
          setPathname('/login/core/first_setup');
          setShowSetupPrompt(false);
        }}
        closeButtonText="Setup"
      >
        This core is not setup yet. Please complete the setup process.
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
