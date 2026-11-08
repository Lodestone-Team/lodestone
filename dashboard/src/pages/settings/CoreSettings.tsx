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
  NO_SCRIPTING_MESSAGE,
  useCoreCapabilities,
  useCoreInfo,
} from 'data/SystemInfo';
import { useUserInfo } from 'data/UserInfo';
import { Form, Formik, FormikHelpers } from 'formik';
import { useContext, useState } from 'react';
import { toast } from 'react-toastify';
import * as yup from 'yup';
import {
  axiosPutSingleValue,
  catchAsyncToString,
  DISABLE_AUTOFILL,
  errorToString,
} from 'utils/util';
import { openPort, stopCli } from 'utils/apis';
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

const tlsModeLabels: Record<TlsMode, string> = {
  self_signed: 'Built-in self-signed certificate',
  user_provided: 'Your own certificate (tls/cert.pem)',
  none: 'Unavailable, plain HTTP only',
};

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
  useDocumentTitle('Lodestone Core Settings - Lodestone');
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
  const codeExecutionCovers = macrosAvailable
    ? 'macros, atom servers, extensions, server scripts and custom Java commands'
    : 'custom Java commands';
  // use a promise to track if safemode is successfully disabled

  const errorString = errorToString(error);
  // TODO: better form error displaying
  const nameField = (
    <InputBox
      label="Core name"
      value={globalSettings?.core_name}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'A nickname for this core. This is what you and others will see when you connect to this core'
      }
      validate={async (name) => {
        // don't be empty
        if (name === '') throw new Error('Name cannot be empty');
        // don't be too long
        if (name.length > 32)
          throw new Error('Name cannot be longer than 32 characters');
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
      label="Public domain or IP"
      value={globalSettings?.domain ?? ''}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        //TODO: more info needed once we add more functionality
        'The domain or public IP address of this core'
      }
      placeholder={`${core?.address}`}
      validate={async (domain) => {
        // can be empty
        if (domain === '') return;
        // don't be too long
        if (domain.length > 253)
          throw new Error('Domain cannot be longer than 253 characters');
      }}
      onSubmit={async (domain) => {
        await axiosPutSingleValue('/global_settings/domain', domain);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          domain: domain,
        });
        notifySaved();
      }}
      tooltipText={
        'This is your public IP if you are port-forwarded. If not, leave this as is.'
      }
    />
  );

  const unsafeModeField = (
    <ToggleBox
      label={'Safe mode'}
      value={globalSettings?.safe_mode ?? false}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'Attempts to prevent non-owner users from accessing adminstrative permissions on your machine'
      }
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
      label={'Code execution'}
      value={codeExecutionEnabled}
      isLoading={isLoading || capabilitiesPending}
      error={errorString}
      disabled={!can_change_core_settings || codeExecutionLocked}
      canRead={userInfo !== undefined}
      description={
        <>
          Allow {codeExecutionCovers} to run on this core.
          {!macrosAvailable && (
            <span className="block italic">{NO_SCRIPTING_MESSAGE}</span>
          )}
          {codeExecutionLocked && (
            <span className="block italic">
              Locked by this core&apos;s administrator
              (LODESTONE_DISABLE_CODE_EXECUTION or --disable-code-execution).
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
      label={'Allow LAN access over plain HTTP'}
      value={allowLanHttp}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'Let other devices connect with http:// instead of https://. Passwords and session tokens then cross your network unencrypted. This machine can always use http://.'
      }
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

  const enablePlayitField = (
    <ToggleBox
      label={'Enable Playit integration'}
      value={globalSettings?.playit_enabled ?? false}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'Enable Playit integration allowing users to open their servers for public facing access without port forwarding'
      }
      onChange={async (value) => {
        await axiosPutSingleValue('/global_settings/playit_enabled', value);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          playit_enabled: value,
        });
        notifySaved();
        await stopCli();
      }}
      optimistic={true}
    />
  );

  const listPubliclySupported =
    typeof globalSettings?.list_instances_publicly === 'boolean';
  const listInstancesPubliclyField = (
    <ToggleBox
      label={'List servers publicly'}
      value={globalSettings?.list_instances_publicly === true}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'Let anyone who can reach this core see the servers you make public below, without signing in, and only what you choose for each. Turning this off hides them all at once. Lodestone Launcher uses this to join them.'
      }
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
                  .typeError('Port must be a number')
                  .required('Port is required')
                  .min(1, 'Port must be between 1 and 65535')
                  .max(65535, 'Port must be between 1 and 65535'),
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
                toast.info(`Port ${values.port} opened`);
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
                    label="Port to open"
                    placeholder="25565"
                    type="number"
                  />
                  <div className="flex flex-row justify-between">
                    <Button
                      onClick={() => setShowOpenPortDialog(false)}
                      label="Cancel"
                    />
                    <Button
                      type="submit"
                      label="Open port"
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
      title="Turn off safe mode?"
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
      confirmButtonText="Turn off"
      onClose={() => setShowSafemodeDialog(false)}
      closeButtonText="Cancel"
      type={'info'}
    >
      Are you sure you want to turn off safe mode? This will allow you to give
      users other than yourself the ability to run potentially dangerous
      commands. Make sure you trust all the users you give these permissions to.
    </ConfirmDialog>
  );

  const codeExecutionDialog = (
    <ConfirmDialog
      title="Turn off code execution?"
      isOpen={showCodeExecutionDialog}
      onConfirm={async () => {
        const error = await catchAsyncToString(setCodeExecution(false));
        if (error) {
          toast.error(error);
          return;
        }
        setShowCodeExecutionDialog(false);
      }}
      confirmButtonText="Turn off"
      onClose={() => setShowCodeExecutionDialog(false)}
      closeButtonText="Cancel"
      type={'danger'}
    >
      {macrosAvailable
        ? 'Running macros and atom servers will be stopped. Macros, atom servers, extensions, server scripts and custom Java commands'
        : 'Custom Java commands'}{' '}
      will not run on this core until you turn code execution back on. Nothing
      is started again automatically.
    </ConfirmDialog>
  );

  const allowLanHttpDialog = (
    <ConfirmDialog
      title="Allow plain HTTP from other devices?"
      isOpen={showAllowLanHttpDialog}
      onConfirm={async () => {
        const error = await catchAsyncToString(setAllowLanHttp(true));
        if (error) {
          toast.error(error);
          return;
        }
        setShowAllowLanHttpDialog(false);
      }}
      confirmButtonText="Allow"
      onClose={() => setShowAllowLanHttpDialog(false)}
      closeButtonText="Cancel"
      type={'danger'}
    >
      Devices on your network will be able to connect over plain HTTP. Passwords
      and session tokens sent that way cross the network unencrypted, so anyone
      else on the network can read them and take over accounts on this core.
      Only allow this if a device cannot use https://.
    </ConfirmDialog>
  );

  const openPortField = (
    <SettingsRow
      label="Open port"
      hint={
        can_change_core_settings ? (
          <>
            Attempt to port forward on your router using{' '}
            <a
              href="https://en.wikipedia.org/wiki/Universal_Plug_and_Play"
              target="_blank"
              rel="noreferrer"
              className="text-blue-200 hover:underline"
            >
              UPnP
            </a>
            . This will allow people on the internet to connect to your server.
          </>
        ) : (
          'No permission'
        )
      }
    >
      <Button
        label="Open port"
        intention="danger"
        disabled={!can_change_core_settings}
        onClick={() => {
          setShowOpenPortDialog(true);
        }}
      />
    </SettingsRow>
  );

  function formatRamSize(totalRam?: number): string {
    if (!totalRam) return 'No ram? How are you running lodestone?';
    const exponent = Math.floor(Math.log2(totalRam) / 10);
    const size = (totalRam / Math.pow(1024, exponent)).toFixed(2);
    const unit = ['B', 'KB', 'MB', 'GB', 'TB'][exponent];
    return `${size} ${unit}`;
  }

  return (
    <>
      {safeModeDialog}
      {codeExecutionDialog}
      {allowLanHttpDialog}
      {openPortModal}
      <SettingsPage title="Core settings">
        {allowLanHttp && (
          <WarningAlert>
            <p>
              Other devices can connect to this core over plain HTTP, so
              passwords and session tokens can cross your network unencrypted.{' '}
              {can_change_core_settings
                ? 'Turn off "Allow LAN access over plain HTTP" below and connect with https:// instead.'
                : 'Connect with https:// and ask the owner to turn off plain HTTP access.'}
            </p>
          </WarningAlert>
        )}
        <SettingsSection
          title="General"
          lead="These settings are for the core itself."
        >
          <SettingsCard>
            {nameField}
            {domainField}
            {enablePlayitField}
          </SettingsCard>
        </SettingsSection>
        {can_change_core_settings && listPubliclySupported && (
          <SettingsSection
            title="Public servers"
            lead="What anyone can see without signing in. Only you can change this."
          >
            <SettingsCard>
              {listInstancesPubliclyField}
              <PublicInstanceRows
                switchOn={globalSettings?.list_instances_publicly === true}
              />
            </SettingsCard>
          </SettingsSection>
        )}
        <SettingsSection
          title="Information"
          lead="This is information about your core and dashboard."
        >
          <SettingsCard>
            <InfoRow label="Core version">{coreInfo?.version}</InfoRow>
            <InfoRow label="Dashboard version">{DASHBOARD_VERSION}</InfoRow>
            <InfoRow label="OS">{coreInfo?.os}</InfoRow>
            <InfoRow label="Architecture">{coreInfo?.arch}</InfoRow>
            <InfoRow label="CPU">{coreInfo?.cpu}</InfoRow>
            <InfoRow label="CPU cores">{coreInfo?.cpu_count}</InfoRow>
            <InfoRow label="Total RAM">
              {formatRamSize(coreInfo?.total_ram)}
            </InfoRow>
            {coreInfo?.tls && (
              <InfoRow label="HTTPS certificate">
                {tlsModeLabels[coreInfo.tls.mode] ?? coreInfo.tls.mode}
              </InfoRow>
            )}
            {coreInfo?.tls?.fingerprint_sha256 && (
              <SettingsRow
                label="Certificate fingerprint (SHA-256)"
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
          title="Danger zone"
          lead="These settings can cause irreversible damage to your core."
          danger
        >
          <SettingsCard danger>
            {unsafeModeField}
            {can_change_core_settings && codeExecutionField}
            {can_change_core_settings &&
              allowLanHttpSupported &&
              allowLanHttpField}
            {openPortField}
          </SettingsCard>
        </SettingsSection>
      </SettingsPage>
    </>
  );
};

export default CoreSettings;
