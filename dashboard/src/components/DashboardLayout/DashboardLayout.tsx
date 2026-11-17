import TopNav from './TopNav';
import { useContext } from 'react';
import { useEventStream } from 'data/EventStream';
import {
  canReachLocalCore,
  useCoreInfo,
  useLocalCoreInfo,
} from 'data/SystemInfo';
import { Suspense, useEffect, useState } from 'react';
import { DashboardBodySkeleton } from './LayoutSkeletons';
import NotificationPanel from './NotificationPanel';
import { useUserInfo } from 'data/UserInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { Outlet } from 'react-router';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { Popover } from '@headlessui/react';
import { DEFAULT_LOCAL_CORE, isLocalCore, isLocalOnlyError } from 'utils/util';
import { LodestoneContext } from 'data/LodestoneContext';
import { major, minor, patch, valid, eq } from 'semver';
import { toast } from 'react-toastify';
import { useLocalStorage } from 'usehooks-ts';
import { DASHBOARD_VERSION } from 'utils/version';
import { useI18n } from 'i18n';
import { KindIcon, VersionTag } from 'components/Atoms/Values';
import {
  CompareTable,
  Fact,
  FactLine,
  ObjectCard,
  ObjectToast,
  ValueField,
} from 'components/Values';

export default function DashboardLayout() {
  const { t, rich } = useI18n();
  const { data: userInfo } = useUserInfo();
  const { setPathname } = useContext(BrowserLocationContext);
  useEventStream();

  /* Start Core */
  const { setCore, addCore, coreConnectionStatus, core } =
    useContext(LodestoneContext);
  const [showSetupPrompt, setShowSetupPrompt] = useState(false);
  const [showLocalSetupPrompt, setShowLocalSetupPrompt] = useState(false);
  const {
    data: coreInfo,
    isPending: coreInfoLoading,
    error: coreInfoError,
  } = useCoreInfo();
  // the core answered, but only allows apps on its own computer
  const localOnly = isLocalOnlyError(coreInfoError);
  // look for a new local core only if it's reachable and not the current core
  const { data: localCoreInfo } = useLocalCoreInfo(
    canReachLocalCore() && !isLocalCore(core)
  );
  const [showVersionMismatchModal, setShowVersionMismatchModal] =
    useState(false);
  const [showThankYouModal, setShowThankYouModal] = useState(false);
  const [showCoreErrorModal, setShowCoreErrorModal] = useState(false);
  const [latestVersion, setLatestVersion] = useLocalStorage(
    'latestVersion',
    '0.0.0'
  );
  const dashboardVersion = DASHBOARD_VERSION;

  // open the error modal is coreConnectionStatus is error for more than 3 seconds
  useEffect(() => {
    setShowCoreErrorModal(coreConnectionStatus === 'error');
  }, [coreConnectionStatus]);

  useEffect(() => {
    if (latestVersion === coreInfo?.version || !coreInfo?.version) return;

    if (
      (coreInfo.version !== latestVersion || latestVersion === '0.0.0') &&
      coreInfo.version.includes('beta')
    )
      setShowThankYouModal(true);

    setLatestVersion(coreInfo.version);
  }, [coreInfo]);

  const thankYouModal = (
    <ConfirmDialog
      title={t('shell.beta.title')}
      type={'info'}
      isOpen={showThankYouModal}
      onClose={() => setShowThankYouModal(false)}
      closeButtonText={t('common.close')}
    >
      {t('shell.beta.body')}
    </ConfirmDialog>
  );

  const versionMismatchModal = !coreInfoLoading && (
    <ConfirmDialog
      title={t('shell.versionMismatch.title')}
      type={'danger'}
      isOpen={showVersionMismatchModal}
      onClose={() => setShowVersionMismatchModal(false)}
      closeButtonText={t('shell.versionMismatch.continue')}
    >
      <CompareTable
        className="mb-4"
        columns={[t('values.key.core'), t('values.key.dashboard')]}
        rows={[
          {
            label: t('values.key.version'),
            a: coreInfo?.version ? (
              <VersionTag version={coreInfo.version} />
            ) : (
              '–'
            ),
            b: <VersionTag version={dashboardVersion} />,
            mismatch: coreInfo?.version !== dashboardVersion,
          },
        ]}
      />
      <p className="text-red-200">{t('shell.versionMismatch.incompatible')}</p>
      {rich('shell.versionMismatch.body', {
        link: (
          <a
            href="https://github.com/Lodestone-Team/lodestone/wiki/Updating"
            className="text-blue-200"
          >
            {t('shell.versionMismatch.wiki')}
          </a>
        ),
      })}
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
          <ObjectToast outcome={t('shell.versionMismatch.toastTitle')}>
            <div className="flex flex-col gap-1">
              <FactLine className="h-auto">
                <Fact label={t('values.key.core')}>
                  <VersionTag version={clientVersion} />
                </Fact>
                <Fact label={t('values.key.dashboard')}>
                  <VersionTag version={dashboardVersion} />
                </Fact>
              </FactLine>
              <span className="text-small text-gray-400">
                {t('shell.versionMismatch.question')}
              </span>
            </div>
          </ObjectToast>,
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
        title={t('shell.localCore.title')}
        type="info"
        confirmButtonText={t('shell.localCore.setup')}
        onConfirm={() => {
          setCore(DEFAULT_LOCAL_CORE);
          setPathname('/login/core/first_setup');
          setShowLocalSetupPrompt(false);
        }}
        closeButtonText={t('shell.localCore.skip')}
        onClose={() => {
          setShowLocalSetupPrompt(false);
        }}
        object={
          localCoreInfo?.core_name && (
            <ObjectCard
              compact
              icon={<KindIcon kind="core" className="w-4" />}
              name={localCoreInfo.core_name}
            />
          )
        }
      >
        {t('shell.localCore.body')}
      </ConfirmDialog>
      <ConfirmDialog
        isOpen={showSetupPrompt}
        title={t('shell.setupRequired.title')}
        type="info"
        z-index="20"
        confirmButtonText={t('shell.localCore.setup')}
        onConfirm={() => {
          setPathname('/login/core/first_setup');
          setShowSetupPrompt(false);
        }}
        closeButtonText={t('shell.setupRequired.changeCore')}
        onClose={() => {
          setPathname('/login/core/select');
          setShowSetupPrompt(false);
        }}
        object={
          coreInfo?.core_name && (
            <ObjectCard
              compact
              icon={<KindIcon kind="core" className="w-4" />}
              name={coreInfo.core_name}
            />
          )
        }
      >
        {t('shell.setupRequired.body')}
      </ConfirmDialog>
      <ConfirmDialog
        isOpen={showCoreErrorModal}
        title={
          localOnly ? t('shell.localOnly.title') : t('shell.coreError.title')
        }
        type="info"
        z-index="20"
        confirmButtonText={t('shell.coreError.back')}
        onConfirm={() => {
          setPathname('/login/core/select');
        }}
        closeButtonText={t('shell.coreError.continue')}
        onClose={() => {
          window.location.reload();
        }}
      >
        {localOnly ? (
          t('common.localOnly')
        ) : (
          <div className="flex flex-col gap-3">
            {t('shell.coreError.body')}
            <ValueField
              label={t('values.key.address')}
              kind="code"
              value={`${core.address}:${core.port}`}
            />
          </div>
        )}
      </ConfirmDialog>
      <div className="flex h-screen flex-col">
        <TopNav />
        <div className="flex min-h-0 w-full grow flex-row bg-gray-875">
          {versionMismatchModal}
          <Suspense fallback={<DashboardBodySkeleton />}>
            <Outlet />
          </Suspense>
        </div>
      </div>
      {thankYouModal}
    </>
  );
}
