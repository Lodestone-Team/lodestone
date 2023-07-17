import { Dialog } from '@headlessui/react';
import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import InputBox from 'components/Atoms/Config/InputBox';
import ToggleBox from 'components/Atoms/Config/ToggleBox';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { useGlobalSettings } from 'data/GlobalSettings';
import { LodestoneContext } from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';
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
import { openPort } from 'utils/apis';
import InputField from 'components/Atoms/Form/InputField';
import { useDocumentTitle } from 'usehooks-ts';

export const CoreSettings = () => {
  useDocumentTitle('Lodestone Core Settings - Lodestone');
  const { core } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const { data: globalSettings, isLoading, error } = useGlobalSettings();
  const { data: coreInfo } = useCoreInfo();
  const { data: userInfo } = useUserInfo();
  const can_change_core_settings = userInfo?.is_owner ?? false;
  const [showSafemodeDialog, setShowSafemodeDialog] = useState(false);
  const [showOpenPortDialog, setShowOpenPortDialog] = useState(false);
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

  const openPortModal = (
    <Dialog
      open={showOpenPortDialog}
      onClose={() => setShowOpenPortDialog(false)}
    >
      <div className="fixed inset-0 bg-[#000]/80" />
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
                  actions.setErrors({ port: errorToString(error) });
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

  const openPortField = (
    <div className="relative flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-h3">
      <div className="flex min-w-0 grow flex-col">
        {can_change_core_settings ? (
          <label className="text-medium font-medium text-gray-300">
            Open Port
          </label>
        ) : (
          <label className="text-medium font-medium text-gray-300">
            Open Port
          </label>
        )}
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

  return (
    <>
      {safeModeDialog}
      {openPortModal}
      <div className="relative mx-auto flex h-full w-full max-w-2xl flex-col @container ">
        <div className="flex w-full flex-col gap-12 overflow-y-scroll px-4 pt-8">
          <h1 className="dashboard-instance-heading">Core Settings</h1>
          <div className="flex w-full flex-col gap-4 @4xl:flex-row">
            <div className="w-[28rem]">
              <h2 className="text-h2 font-bold tracking-medium">
                General Settings
              </h2>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                These settings are for the core itself.
              </h3>
            </div>
            <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
              {nameField}
              {domainField}
            </div>
          </div>
          <div className="flex w-full flex-col gap-4 @4xl:flex-row">
            <div className="w-[28rem]">
              <h2 className="text-h2 font-bold tracking-medium">Danger Zone</h2>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                These settings can cause irreversible damage to your server!
              </h3>
            </div>
            <div className="w-full rounded-lg border border-red-faded child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
              {unsafeModeField}
              {openPortField}
            </div>
          </div>
        </div>
      </div>
    </>
  );
};

export default CoreSettings;
