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
import { useUserInfo, useUserLoggedIn } from 'data/UserInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { Outlet } from 'react-router-dom';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { Popover } from '@headlessui/react';
import { DEFAULT_LOCAL_CORE, LODESTONE_PORT } from 'utils/util';
import { LodestoneContext } from 'data/LodestoneContext';

export default function DashboardLayout() {
  const { data: userInfo } = useUserInfo();
  const { setPathname } = useContext(BrowserLocationContext);
  useEventStream();

  /* Start Notification */
  const [rightNavSize, setRightNavSize] = useLocalStorage('rightNavSize', 480);
  const { width } = useWindowSize();
  /* End Notification */

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
    <>
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
        There was an error connecting to {core.address}:{core.port}. Please
        select a different core, refresh the page, or simply wait for the core
        to come back online.
      </ConfirmDialog>
      <Popover className="relative flex h-screen flex-col">
        <TopNav
        // showNotifications={showNotifications}
        // setShowNotifications={setShowNotifications}
        />
        <div className="relative flex min-h-0 w-full grow flex-row bg-gray-850">
          <Outlet />

          <div className="absolute right-[5.25rem] top-10 flex h-[80vh] flex-row">
            <Popover.Panel
              className="h-full rounded-lg drop-shadow-lg child:h-full"
              style={{
                width: rightNavSize,
              }}
            >
              <NotificationPanel className="rounded-lg border" />
            </Popover.Panel>
            {/* very scuff way to align the notification panel with icon */}
            <div className="opacity-none pointer-events-none -z-10 select-none">
              {userInfo?.username ? `Hi, ${userInfo.username}` : 'Not logg'}
            </div>
          </div>
        </div>
      </Popover>
    </>
  );
}
