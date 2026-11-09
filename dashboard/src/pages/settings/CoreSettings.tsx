import { Dialog } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import InputBox from 'components/Atoms/Config/InputBox';
import ToggleBox from 'components/Atoms/Config/ToggleBox';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { useGlobalSettings } from 'data/GlobalSettings';
import { useCodeExecution } from 'data/CodeExecution';
import { GlobalSettingsData } from 'bindings/GlobalSettingsData';
import { LodestoneContext } from 'data/LodestoneContext';
import {
  noScriptingMessage,
  useCoreCapabilities,
  useCoreInfo,
} from 'data/SystemInfo';
import { useUserInfo } from 'data/UserInfo';
import { Form, Formik, FormikHelpers } from 'formik';
import { MessageKey, useI18n } from 'i18n';
import { useContext, useState } from 'react';
import { toast } from 'react-toastify';
import * as yup from 'yup';
import {
  axiosPutSingleValue,
  catchAsyncToString,
  DISABLE_AUTOFILL,
  errorToString,
} from 'utils/util';
import { openPort } from 'utils/apis';
import { DASHBOARD_VERSION } from 'utils/version';
import InputField from 'components/Atoms/Form/InputField';
import { useDocumentTitle } from 'usehooks-ts';
import WarningAlert from 'components/Atoms/WarningAlert';
import { TlsMode } from 'bindings/TlsMode';
import { PublicInstanceRows } from 'components/Instance/PublicListingSetting';
import {
  SettingsCard,
  SettingsPage,
  SettingsRow,
  SettingsSection,
  SettingsValue,
} from 'components/Settings/SettingsBlocks';
import { notifySaved } from 'components/Settings/savedToast';

const tlsModeLabels = {
  self_signed: 'settings.core.tls.selfSigned',
  user_provided: 'settings.core.tls.userProvided',
  none: 'settings.core.tls.none',
} as const satisfies Record<TlsMode, MessageKey>;

const CORE_NAME_MAX_LENGTH = 32;
const DOMAIN_MAX_LENGTH = 253;

// a read-only row in the Information section
const InfoRow = ({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) => (
  <SettingsRow label={label} controlClassName="min-w-0 shrink">
    <SettingsValue className="wrap-break-word">{children}</SettingsValue>
  </SettingsRow>
);

export const CoreSettings = () => {
  const { t, rich, formatBytes } = useI18n();
  useDocumentTitle(
    t('common.pageTitle', { page: t('settings.core.documentTitle') })
  );
  const { core } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const {
    data: globalSettings,
    isPending: isLoading,
    error,
  } = useGlobalSettings();
  const { data: coreInfo } = useCoreInfo();
  const { data: userInfo } = useUserInfo();
  const can_change_core_settings = userInfo?.is_owner ?? false;
  const [showSafemodeDialog, setShowSafemodeDialog] = useState(false);
  const [showOpenPortDialog, setShowOpenPortDialog] = useState(false);
  const [showCodeExecutionDialog, setShowCodeExecutionDialog] = useState(false);
  const [showAllowLanHttpDialog, setShowAllowLanHttpDialog] = useState(false);
  // older cores have no such setting
  const allowLanHttpSupported =
    typeof globalSettings?.allow_lan_http === 'boolean';
  const allowLanHttp = globalSettings?.allow_lan_http === true;
  const { enabled: codeExecutionEnabled, locked: codeExecutionLocked } =
    useCodeExecution();
  // without Deno, code execution only covers custom Java commands
  const { macros: macrosAvailable, isPending: capabilitiesPending } =
    useCoreCapabilities();
  // use a promise to track if safemode is successfully disabled

  const errorString = errorToString(error);
  // TODO: better form error displaying
  const nameField = (
    <InputBox
      label={t('settings.core.name.label')}
      value={globalSettings?.core_name}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      validate={async (name) => {
        // don't be empty
        if (name === '') throw new Error(t('settings.core.name.empty'));
        // don't be too long
        if (name.length > CORE_NAME_MAX_LENGTH)
          throw new Error(
            t('settings.core.name.tooLong', { max: CORE_NAME_MAX_LENGTH })
          );
      }}
      onSubmit={async (name) => {
        await axiosPutSingleValue('/global_settings/name', name);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          core_name: name,
        });
        queryClient.setQueryData(['systeminfo', 'CoreInfo'], {
          ...coreInfo,
          core_name: name,
        });
        notifySaved();
      }}
    />
  );

  const domainField = (
    <InputBox
      label={t('settings.core.domain.label')}
      value={globalSettings?.domain ?? ''}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      placeholder={`${core?.address}`}
      validate={async (domain) => {
        // can be empty
        if (domain === '') return;
        // don't be too long
        if (domain.length > DOMAIN_MAX_LENGTH)
          throw new Error(
            t('settings.core.domain.tooLong', { max: DOMAIN_MAX_LENGTH })
          );
      }}
      onSubmit={async (domain) => {
        await axiosPutSingleValue('/global_settings/domain', domain);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          domain: domain,
        });
        notifySaved();
      }}
      tooltipText={t('settings.core.domain.tooltip')}
    />
  );

  const unsafeModeField = (
    <ToggleBox
      label={t('settings.core.safeMode.label')}
      value={globalSettings?.safe_mode ?? false}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={t('settings.core.safeMode.hint')}
      onChange={async (value) => {
        if (value) {
          await axiosPutSingleValue('/global_settings/safe_mode', value);
          queryClient.setQueryData(['global_settings'], {
            ...globalSettings,
            safe_mode: value,
          });
          notifySaved();
        } else {
          setShowSafemodeDialog(true);
        }
      }}
      optimistic={false}
    />
  );

  const setCodeExecution = async (value: boolean) => {
    await axiosPutSingleValue('/global_settings/code_execution', value);
    queryClient.setQueryData(
      ['global_settings'],
      (old: GlobalSettingsData | undefined) =>
        old && { ...old, code_execution_enabled: value }
    );
    notifySaved();
    // the core decides the final state (e.g. a lock), so read it back
    queryClient.invalidateQueries({ queryKey: ['global_settings'] });
  };

  const codeExecutionField = (
    <ToggleBox
      label={t('settings.core.codeExecution.label')}
      value={codeExecutionEnabled}
      isLoading={isLoading || capabilitiesPending}
      error={errorString}
      disabled={!can_change_core_settings || codeExecutionLocked}
      canRead={userInfo !== undefined}
      description={
        <>
          {macrosAvailable
            ? t('settings.core.codeExecution.hint')
            : t('settings.core.codeExecution.hintJavaOnly')}
          {!macrosAvailable && (
            <span className="block italic">{noScriptingMessage()}</span>
          )}
          {codeExecutionLocked && (
            <span className="block italic">
              {t('settings.core.codeExecution.locked')}
            </span>
          )}
        </>
      }
      onChange={async (value) => {
        if (value) {
          await setCodeExecution(true);
        } else {
          setShowCodeExecutionDialog(true);
        }
      }}
      optimistic={false}
    />
  );

  const setAllowLanHttp = async (value: boolean) => {
    await axiosPutSingleValue('/global_settings/allow_lan_http', value);
    queryClient.setQueryData(
      ['global_settings'],
      (old: GlobalSettingsData | undefined) =>
        old && { ...old, allow_lan_http: value }
    );
    notifySaved();
  };

  const allowLanHttpField = (
    <ToggleBox
      label={t('settings.core.lanHttp.label')}
      value={allowLanHttp}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={t('settings.core.lanHttp.hint')}
      onChange={async (value) => {
        if (value) {
          setShowAllowLanHttpDialog(true);
        } else {
          await setAllowLanHttp(false);
        }
      }}
      optimistic={false}
    />
  );

  const localClaimSupported =
    typeof globalSettings?.allow_local_claim === 'boolean';
  const allowLocalClaimField = (
    <ToggleBox
      label={t('settings.core.localClaim.label')}
      value={globalSettings?.allow_local_claim === true}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={t('settings.core.localClaim.hint')}
      onChange={async (value) => {
        await axiosPutSingleValue('/global_settings/allow_local_claim', value);
        queryClient.setQueryData(
          ['global_settings'],
          (old: GlobalSettingsData | undefined) =>
            old && { ...old, allow_local_claim: value }
        );
        notifySaved();
      }}
      optimistic={false}
    />
  );

  const listPubliclySupported =
    typeof globalSettings?.list_instances_publicly === 'boolean';
  const listInstancesPubliclyField = (
    <ToggleBox
      label={t('settings.core.listPublicly.label')}
      value={globalSettings?.list_instances_publicly === true}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={t('settings.core.listPublicly.hint')}
      onChange={async (value) => {
        await axiosPutSingleValue(
          '/global_settings/list_instances_publicly',
          value
        );
        queryClient.setQueryData(
          ['global_settings'],
          (old: GlobalSettingsData | undefined) =>
            old && { ...old, list_instances_publicly: value }
        );
        notifySaved();
      }}
      optimistic={false}
    />
  );

  const openPortModal = (
    <Dialog
      open={showOpenPortDialog}
      onClose={() => setShowOpenPortDialog(false)}
      className="relative z-60"
    >
      <ModalBackdrop strong />
      <div className="fixed inset-0 overflow-y-auto">
        <div className="flex min-h-full items-center justify-center p-4 text-center">
          <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-800 px-8 pt-16 pb-8">
            <Formik
              initialValues={{ port: 25565 }}
              validationSchema={yup.object({
                port: yup
                  .number()
                  .typeError(() => t('settings.core.openPort.portNotNumber'))
                  .required(() => t('settings.core.openPort.portRequired'))
                  .min(1, () => t('settings.core.openPort.portRange'))
                  .max(65535, () => t('settings.core.openPort.portRange')),
              })}
              onSubmit={async (
                values: { port: number },
                actions: FormikHelpers<{ port: number }>
              ) => {
                actions.setSubmitting(true);
                const error = await openPort(values.port);
                actions.setSubmitting(false);
                if (error) {
                  // keep the dialog open with the error instead of reporting success
                  actions.setErrors({ port: errorToString(error) });
                  return;
                }
                actions.resetForm();
                setShowOpenPortDialog(false);
                toast.info(
                  t('settings.core.openPort.opened', { port: values.port })
                );
              }}
            >
              {({ isSubmitting }) => (
                <Form
                  id="open-port-form"
                  autoComplete={DISABLE_AUTOFILL}
                  className="flex flex-col items-stretch gap-8 text-center"
                >
                  <InputField
                    name="port"
                    label={t('settings.core.openPort.portLabel')}
                    placeholder="25565"
                    type="number"
                  />
                  <div className="flex flex-row justify-between gap-2">
                    <Button
                      onClick={() => setShowOpenPortDialog(false)}
                      label={t('common.cancel')}
                    />
                    <Button
                      type="submit"
                      label={t('settings.core.openPort.button')}
                      loading={isSubmitting}
                    />
                  </div>
                </Form>
              )}
            </Formik>
          </ModalPanel>
        </div>
      </div>
    </Dialog>
  );

  const safeModeDialog = (
    <ConfirmDialog
      title={t('settings.core.safeMode.offTitle')}
      isOpen={showSafemodeDialog}
      onConfirm={async () => {
        const error = await catchAsyncToString(
          axiosPutSingleValue('/global_settings/safe_mode', false)
        );
        if (error) {
          toast.error(error);
          return;
        }
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          safe_mode: false,
        });
        notifySaved();
        setShowSafemodeDialog(false);
      }}
      confirmButtonText={t('settings.core.turnOff')}
      onClose={() => setShowSafemodeDialog(false)}
      closeButtonText={t('common.cancel')}
      type={'info'}
    >
      {t('settings.core.safeMode.offBody')}
    </ConfirmDialog>
  );

  const codeExecutionDialog = (
    <ConfirmDialog
      title={t('settings.core.codeExecution.offTitle')}
      isOpen={showCodeExecutionDialog}
      onConfirm={async () => {
        const error = await catchAsyncToString(setCodeExecution(false));
        if (error) {
          toast.error(error);
          return;
        }
        setShowCodeExecutionDialog(false);
      }}
      confirmButtonText={t('settings.core.turnOff')}
      onClose={() => setShowCodeExecutionDialog(false)}
      closeButtonText={t('common.cancel')}
      type={'danger'}
    >
      {macrosAvailable
        ? t('settings.core.codeExecution.offBody')
        : t('settings.core.codeExecution.offBodyJavaOnly')}
    </ConfirmDialog>
  );

  const allowLanHttpDialog = (
    <ConfirmDialog
      title={t('settings.core.lanHttp.confirmTitle')}
      isOpen={showAllowLanHttpDialog}
      onConfirm={async () => {
        const error = await catchAsyncToString(setAllowLanHttp(true));
        if (error) {
          toast.error(error);
          return;
        }
        setShowAllowLanHttpDialog(false);
      }}
      confirmButtonText={t('settings.core.lanHttp.allow')}
      onClose={() => setShowAllowLanHttpDialog(false)}
      closeButtonText={t('common.cancel')}
      type={'danger'}
    >
      {t('settings.core.lanHttp.confirmBody')}
    </ConfirmDialog>
  );

  const openPortField = (
    <SettingsRow
      label={t('settings.core.openPort.label')}
      hint={
        can_change_core_settings
          ? rich('settings.core.openPort.hint', {
              upnp: (
                <a
                  href="https://en.wikipedia.org/wiki/Universal_Plug_and_Play"
                  target="_blank"
                  rel="noreferrer"
                  className="text-blue-200 hover:underline"
                >
                  UPnP
                </a>
              ),
            })
          : t('settings.core.openPort.noPermission')
      }
    >
      <Button
        label={t('settings.core.openPort.button')}
        intention="danger"
        className="whitespace-nowrap"
        disabled={!can_change_core_settings}
        onClick={() => {
          setShowOpenPortDialog(true);
        }}
      />
    </SettingsRow>
  );

  const formatRamSize = (totalRam?: number): string =>
    totalRam ? formatBytes(totalRam) : t('settings.core.info.noRam');

  return (
    <>
      {safeModeDialog}
      {codeExecutionDialog}
      {allowLanHttpDialog}
      {openPortModal}
      <SettingsPage title={t('settings.core.title')}>
        {allowLanHttp && (
          <WarningAlert>
            <p>
              {can_change_core_settings
                ? t('settings.core.lanHttp.warningOwner', {
                    setting: t('settings.core.lanHttp.label'),
                  })
                : t('settings.core.lanHttp.warningOther')}
            </p>
          </WarningAlert>
        )}
        <SettingsSection title={t('settings.core.section.core')}>
          <SettingsCard>
            {nameField}
            {domainField}
          </SettingsCard>
        </SettingsSection>
        {can_change_core_settings && listPubliclySupported && (
          <SettingsSection
            title={t('settings.core.section.public')}
            lead={t('settings.core.section.publicLead')}
          >
            <SettingsCard>
              {listInstancesPubliclyField}
              <PublicInstanceRows
                switchOn={globalSettings?.list_instances_publicly === true}
              />
            </SettingsCard>
          </SettingsSection>
        )}
        <SettingsSection title={t('settings.core.section.info')}>
          <SettingsCard>
            <InfoRow label={t('settings.core.info.coreVersion')}>
              {coreInfo?.version}
            </InfoRow>
            <InfoRow label={t('settings.core.info.dashboardVersion')}>
              {DASHBOARD_VERSION}
            </InfoRow>
            <InfoRow label={t('settings.core.info.os')}>{coreInfo?.os}</InfoRow>
            <InfoRow label={t('settings.core.info.arch')}>
              {coreInfo?.arch}
            </InfoRow>
            <InfoRow label={t('settings.core.info.cpu')}>
              {coreInfo?.cpu}
            </InfoRow>
            <InfoRow label={t('settings.core.info.cpuCores')}>
              {coreInfo?.cpu_count}
            </InfoRow>
            <InfoRow label={t('settings.core.info.totalRam')}>
              {formatRamSize(coreInfo?.total_ram)}
            </InfoRow>
            {coreInfo?.tls && (
              <InfoRow label={t('settings.core.info.httpsCertificate')}>
                {coreInfo.tls.mode in tlsModeLabels
                  ? t(tlsModeLabels[coreInfo.tls.mode])
                  : coreInfo.tls.mode}
              </InfoRow>
            )}
            {coreInfo?.tls?.fingerprint_sha256 && (
              <SettingsRow
                label={t('settings.core.info.fingerprint')}
                hint={
                  <span className="font-mono break-all">
                    {coreInfo.tls.fingerprint_sha256}
                  </span>
                }
              />
            )}
          </SettingsCard>
        </SettingsSection>
        <SettingsSection
          title={t('settings.core.section.danger')}
          lead={t('settings.core.section.dangerLead')}
          danger
        >
          <SettingsCard danger>
            {unsafeModeField}
            {can_change_core_settings && codeExecutionField}
            {can_change_core_settings &&
              allowLanHttpSupported &&
              allowLanHttpField}
            {can_change_core_settings &&
              localClaimSupported &&
              allowLocalClaimField}
            {openPortField}
          </SettingsCard>
        </SettingsSection>
      </SettingsPage>
    </>
  );
};

export default CoreSettings;
