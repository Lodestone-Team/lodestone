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
  const [showMajorVersionModal, setShowMajorVersionModal] = useState(false);
  const dashboardVersion = packageinfo.version;

  const versionMismatchModal = !coreInfoLoading && (
    <ConfirmDialog
      title={`Major Version Mismatch`}
      type={'danger'}
      isOpen={showMajorVersionModal}
      onClose={() => setShowMajorVersionModal(false)}
    >
      <div>
        <b>Core Version: </b>
        {coreInfo?.version}
        <br />
        <b>Dashboard Version: </b>
        {dashboardVersion}
      </div>
      <br />
      Your dashboard and core have a major version mismatch! Please consider
      updating to stay up to date with our latest changes.
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
        setShowMajorVersionModal(true);
      else if (minor(clientVersion) !== minor(dashboardVersion))
        toast.warn(
          `There is a minor version mismatch! Core: ${clientVersion}, Dashboard: ${dashboardVersion}`,
          { toastId: 'minorVersionMismatch' }
        );
      else if (patch(clientVersion) !== patch(dashboardVersion))
        toast.warn(
          `There is a patch version mismatch! Core: ${clientVersion}, Dashboard: ${dashboardVersion}`,
          { toastId: 'patchVersionMismatch' }
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
