import TopNav from './TopNav';
import { useContext } from 'react';
import { useEventStream } from 'data/EventStream';
import { useCoreInfo, useLocalCoreInfo } from 'data/SystemInfo';
import { useEffect, useState } from 'react';
import NotificationPanel from './NotificationPanel';
import { useUserInfo } from 'data/UserInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { Outlet } from 'react-router-dom';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { Popover } from '@headlessui/react';
import { DEFAULT_LOCAL_CORE } from 'utils/util';
import { LodestoneContext } from 'data/LodestoneContext';
import { major, minor, patch, valid, eq } from 'semver';
import { toast } from 'react-toastify';
import packageinfo from '../../../package.json';

export default function DashboardLayout() {
  const { data: userInfo } = useUserInfo();
  const { setPathname } = useContext(BrowserLocationContext);
  useEventStream();

  /* Start Core */
  const { setCore, addCore, coreConnectionStatus, core } =
    useContext(LodestoneContext);
  const [showSetupPrompt, setShowSetupPrompt] = useState(false);
  const [showLocalSetupPrompt, setShowLocalSetupPrompt] = useState(false);
  const { data: coreInfo, isLoading: coreInfoLoading } = useCoreInfo();
  const { data: localCoreInfo } = useLocalCoreInfo();
  const [showVersionMismatchModal, setShowVersionMismatchModal] =
    useState(false);
  const [showCoreErrorModal, setShowCoreErrorModal] = useState(false);
  const dashboardVersion = packageinfo.version;

  // open the error modal is coreConnectionStatus is error for more than 3 seconds
  useEffect(() => {
    if (coreConnectionStatus === 'error') {
      const timeout = setTimeout(() => {
        setShowCoreErrorModal(true);
      }, 3000);
      return () => clearTimeout(timeout);
    } else {
      setShowCoreErrorModal(false);
    }
  }, [coreConnectionStatus]);

  const versionMismatchModal = !coreInfoLoading && (
    <ConfirmDialog
      title={`Update Required!`}
      type={'danger'}
      isOpen={showVersionMismatchModal}
      onClose={() => setShowVersionMismatchModal(false)}
      closeButtonText={'I understand, continue without updating'}
    >
      <div>
        <b>Core Version: </b>
        {coreInfo?.version}
        <br />
        <b>Dashboard Version: </b>
        {dashboardVersion}
      </div>
      <br />
      <p className="text-red-200">Your dashboard and core is incompatible!</p>
      This can cause unexpected behavior. Please update your core to the latest
      version. Visit{' '}
      <a
        href="https://github.com/Lodestone-Team/lodestone/wiki/Updating"
        className="text-blue-200"
      >
        the wiki
      </a>{' '}
      for more information.
    </ConfirmDialog>
  );

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

  useEffect(() => {
    const clientVersion = coreInfoLoading ? undefined : coreInfo?.version;
    if (clientVersion === undefined) return;
    if (valid(clientVersion) && valid(dashboardVersion)) {
      if (eq(clientVersion, dashboardVersion)) return;
      if (major(clientVersion) !== major(dashboardVersion))
        setShowVersionMismatchModal(true);
      else if (minor(clientVersion) !== minor(dashboardVersion))
        // toast.warn(
        //   `There is a minor version mismatch! Core: ${clientVersion}, Dashboard: ${dashboardVersion}`,
        //   { toastId: 'minorVersionMismatch' }
        // );
        setShowVersionMismatchModal(true);
      else if (
        major(clientVersion) === 0 &&
        minor(clientVersion) === 4 &&
        patch(clientVersion) < 4
      )
        setShowVersionMismatchModal(true);
      else if (patch(clientVersion) !== patch(dashboardVersion))
        toast.warn(
          `Version mismatch! Is your core out of date? Core: ${clientVersion}, Dashboard: ${dashboardVersion}`,
          {
            toastId: 'patchVersionMismatch',
            autoClose: 10000,
            position: 'top-center',
          }
        );
    }
  }, [coreInfo?.version]);

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
        isOpen={showCoreErrorModal}
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
      <div className="flex h-screen flex-col">
        <TopNav />
        <div className="flex min-h-0 w-full grow flex-row bg-gray-875">
          {versionMismatchModal}
          <Outlet />
        </div>
      </div>
    </>
  );
}
