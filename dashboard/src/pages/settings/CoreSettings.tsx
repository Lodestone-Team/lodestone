import { Dialog } from '@headlessui/react';
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

const tlsModeLabels: Record<TlsMode, string> = {
  self_signed: 'Built-in self-signed certificate',
  user_provided: 'Your own certificate (tls/cert.pem)',
  none: 'Unavailable, plain HTTP only',
};

// a read-only row in the Information section
const InfoRow = ({ children }: { children: React.ReactNode }) => (
  <div className="relative flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-h3">
    <div className="flex min-w-0 grow flex-col">
      <label className="text-medium font-medium text-gray-300">
        {children}
      </label>
    </div>
  </div>
);

export const CoreSettings = () => {
  useDocumentTitle('Lodestone Core Settings - Lodestone');
  const { core } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const { data: globalSettings, isPending: isLoading, error } = useGlobalSettings();
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
    ? 'macros, atom instances, extensions, instance scripts and custom Java commands'
    : 'custom Java commands';
  // use a promise to track if safemode is successfully disabled

  const errorString = errorToString(error);
  // TODO: better form error displaying
  const nameField = (
    <InputBox
      label="Core Name"
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
      }}
    />
  );

  const domainField = (
    <InputBox
      label="Public Domain/IP"
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
      }}
      tooltipText={"This is your public IP if you are port-forwarded. If not, leave this as is."}
    />
  );

  const unsafeModeField = (
    <ToggleBox
      label={'Safe Mode'}
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
    // the core decides the final state (e.g. a lock), so read it back
    queryClient.invalidateQueries({ queryKey: ['global_settings'] });
  };

  const codeExecutionField = (
    <ToggleBox
      label={'Code Execution'}
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
              Locked by the server administrator
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
      label={'Enable Playit Integration'}
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
        await stopCli();
      }}
      optimistic={true}
    />
  );


  const listPubliclySupported =
    typeof globalSettings?.list_instances_publicly === 'boolean';
  const listInstancesPubliclyField = (
    <ToggleBox
      label={'List instances publicly'}
      value={globalSettings?.list_instances_publicly === true}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'Let anyone who can reach this core see the instances you mark as "Listed publicly" (name, version, loader, status and player count) and download their client mods, without signing in. Lodestone Launcher uses this to join them.'
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
      }}
      optimistic={false}
    />
  );

  const openPortModal = (
    <Dialog
      open={showOpenPortDialog}
      onClose={() => setShowOpenPortDialog(false)}
    >
      <div className="fixed inset-0 bg-scrim-strong" />
      <div className="fixed inset-0 overflow-y-auto">
        <div className="flex min-h-full items-center justify-center p-4 text-center">
          <Dialog.Panel className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-800 px-8 pb-8 pt-16">
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
                      label="Open Port"
                      loading={isSubmitting}
                    />
                  </div>
                </Form>
              )}
            </Formik>
          </Dialog.Panel>
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
        ? 'Running macros and atom instances will be stopped. Macros, atom instances, extensions, instance scripts and custom Java commands'
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
    <div className="relative flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-h3">
      <div className="flex min-w-0 grow flex-col">
        <label className="text-medium font-medium text-gray-300">
          Open Port
        </label>
        {can_change_core_settings ? (
          <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
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
          </div>
        ) : (
          <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
            No permission
          </div>
        )}
      </div>
      <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
        <Button
          label="Open Port"
          intention="danger"
          disabled={!can_change_core_settings}
          onClick={() => {
            setShowOpenPortDialog(true);
          }}
        />
      </div>
    </div>
  );

  function formatRamSize(totalRam?: number): string {
    if (!totalRam) return "No ram? How are you running lodestone?";
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
      <div className="relative mx-auto flex h-full w-full max-w-2xl flex-col @container ">
        <div className="flex w-full flex-col gap-12 overflow-y-scroll px-4 pt-8">
          <h1 className="dashboard-instance-heading">Core Settings</h1>
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
          <div className="flex w-full flex-col gap-4 @4xl:flex-row">
            <div className="w-[28rem]">
              <h2 className="text-h2 font-bold tracking-medium">
                General Settings
              </h2>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                These settings are for the core itself.
              </h3>
            </div>
            <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
              {nameField}
              {domainField}
              {enablePlayitField}
              {can_change_core_settings &&
                listPubliclySupported &&
                listInstancesPubliclyField}
            </div>
          </div>
          <div className="flex w-full flex-col gap-4 @4xl:flex-row">
            <div className="w-[28rem]">
              <h2 className="text-h2 font-bold tracking-medium">Danger Zone</h2>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                These settings can cause irreversible damage to your server!
              </h3>
            </div>
            <div className="mb-10 w-full rounded-lg border border-red-faded child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
              {unsafeModeField}
              {can_change_core_settings && codeExecutionField}
              {can_change_core_settings &&
                allowLanHttpSupported &&
                allowLanHttpField}
              {openPortField}
            </div>
          </div>
          <div className="flex w-full flex-col gap-4 @4xl:flex-row">
            <div className="w-[28rem]">
              <h2 className="text-h2 font-bold tracking-medium">
                Information
              </h2>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                This is information about your core and dashboard
              </h3>
            </div>
            <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
              <InfoRow>Core version: {coreInfo?.version}</InfoRow>
              <InfoRow>Dashboard version: {DASHBOARD_VERSION}</InfoRow>
              <InfoRow>OS: {coreInfo?.os}</InfoRow>
              <InfoRow>Architecture: {coreInfo?.arch}</InfoRow>
              <InfoRow>CPU: {coreInfo?.cpu}</InfoRow>
              <InfoRow>CPU Cores: {coreInfo?.cpu_count}</InfoRow>
              <InfoRow>Total RAM: {formatRamSize(coreInfo?.total_ram)}</InfoRow>
              {coreInfo?.tls && (
                <InfoRow>
                  HTTPS certificate:{' '}
                  {tlsModeLabels[coreInfo.tls.mode] ?? coreInfo.tls.mode}
                </InfoRow>
              )}
              {coreInfo?.tls?.fingerprint_sha256 && (
                <InfoRow>
                  Certificate fingerprint (SHA-256):{' '}
                  <span className="font-mono text-small break-all">
                    {coreInfo.tls.fingerprint_sha256}
                  </span>
                </InfoRow>
              )}
            </div>
          </div>
        </div>
      </div>
    </>
  );
};

export default CoreSettings;
