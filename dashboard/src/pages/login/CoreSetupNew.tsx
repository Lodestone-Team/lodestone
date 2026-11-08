import axios from 'axios';
import Button from 'components/Atoms/Button';
import { useContext, useEffect, useState } from 'react';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import { LodestoneContext } from 'data/LodestoneContext';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { useCoreInfo } from 'data/SystemInfo';
import { useDocumentTitle } from 'usehooks-ts';
import { useTauri } from 'utils/tauriUtil';
import { useQueryClient } from '@tanstack/react-query';
import { LoginReply } from 'bindings/LoginReply';
import WarningAlert from 'components/Atoms/WarningAlert';
import useAnalyticsEventTracker, { useEffectOnce } from 'utils/hooks';
import Logo from 'components/Atoms/Logo';
import { t as translate, useI18n } from 'i18n';

type SetupOwnerFormValues = {
  username: string;
  password: string;
  passwordConfirm: string;
  setupKey: string;
};

const validationSchema = yup.object({
  username: yup
    .string()
    .required(() => translate('shell.login.usernameRequired')),
  password: yup
    .string()
    .required(() => translate('shell.login.passwordRequired')),
  passwordConfirm: yup
    .string()
    .required(() => translate('shell.coreSetup.passwordConfirmRequired'))
    .oneOf([yup.ref('password')], () =>
      translate('shell.coreSetup.passwordMismatch')
    ),
  setupKey: yup
    .string()
    .required(() => translate('shell.coreSetup.setupKeyRequired')),
});

const CoreSetupNew = () => {
  const { t, rich } = useI18n();
  useDocumentTitle(
    t('common.pageTitle', { page: t('shell.coreSetup.pageTitle') })
  );
  const { navigateBack, setPathname } = useContext(BrowserLocationContext);
  const { data: coreInfo } = useCoreInfo(3000);
  const { setToken, core } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const [setupKey, setSetupKey] = useState<string>('');
  const { address, port } = core;
  const { core_name } = coreInfo ?? {};
  const tauri = useTauri();
  const socket = `${address}:${port}`;
  const keyPrefilled = !!(tauri && setupKey);
  const gaEventTracker = useAnalyticsEventTracker('Core Setup Page');

  useEffectOnce(() => {
    if (!tauri) return;
    tauri?.core
      .invoke<string | null>('get_first_time_setup_key')
      .then((setup_key) => {
        setSetupKey(setup_key ?? '');
      })
      .catch((err: any) => {
        console.log('Tauri call failed get_first_time_setup_key', err);
      });
  });

  useEffect(() => {
    if (coreInfo?.is_setup) {
      setPathname('/login/user/select');
    }
  }, [coreInfo]);

  const initialValues: SetupOwnerFormValues = {
    username: '',
    password: '',
    passwordConfirm: '',
    setupKey,
  };

  const onSubmit = (
    values: SetupOwnerFormValues,
    actions: FormikHelpers<SetupOwnerFormValues>
  ) => {
    // check if core can be reached
    axios
      .post<LoginReply>(`/setup/${values.setupKey}`, {
        username: values.username,
        password: values.password,
      })
      .then((res) => {
        if (res.status !== 200) throw new Error(t('shell.coreSetup.failed'));
        return res.data;
      })
      .then((res) => {
        setToken(res.token, socket);
        setPathname('/login/core/first_config');
        gaEventTracker('Setup Owner Account');
        queryClient.invalidateQueries();
        actions.setSubmitting(false);
      })
      .catch((err) => {
        const errorMessages = errorToString(err);
        actions.setStatus({ error: errorMessages });
        actions.setSubmitting(false);
        return;
      });
  };

  return (
    <div className="@container flex w-[768px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl px-12 py-14">
      <div className="text flex flex-col items-start">
        <Logo alt="Lodestone" className="h-8" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          {t('shell.coreSetup.title')}
        </h1>
        <h2 className="text-medium font-medium tracking-medium text-white/50">
          {core_name} ({socket})
        </h2>
        {keyPrefilled ? null : (
          <h2 className="text-medium font-medium tracking-medium text-white/50">
            {t('shell.coreSetup.findKey')}
          </h2>
        )}
      </div>
      <Formik
        initialValues={initialValues}
        validationSchema={validationSchema}
        onSubmit={onSubmit}
        validateOnBlur={false}
        validateOnChange={false}
        enableReinitialize={true}
      >
        {({ isSubmitting, status }) => (
          <Form
            id="setupOwnerForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            {status && (
              <WarningAlert>
                <p>
                  {rich('shell.login.checkFields', {
                    error: <b>{status.error}</b>,
                  })}
                </p>
              </WarningAlert>
            )}
            <div className="grid grid-cols-1 gap-x-8 gap-y-14 @lg:grid-cols-2">
              <InputField
                type="text"
                name="username"
                label={t('shell.login.username')}
              />
              <InputField
                type="password"
                name="setupKey"
                label={
                  keyPrefilled
                    ? t('shell.coreSetup.setupKeyPrefilled')
                    : t('shell.coreSetup.setupKey')
                }
                disabled={keyPrefilled}
              />
              <InputField
                type="password"
                name="password"
                label={t('shell.login.password')}
              />
              <InputField
                type="password"
                name="passwordConfirm"
                label={t('shell.coreSetup.confirmPassword')}
              />
            </div>
            <div className="flex w-full flex-row justify-end gap-4">
              {/* <Button
                type="button"
                iconRight={faArrowLeft}
                label="Back"
                onClick={navigateBack}
              /> */}
              <Button
                type="submit"
                intention="primary"
                label={t('shell.coreSetup.submit')}
                iconRight={faArrowRight}
                loading={isSubmitting}
              />
            </div>
          </Form>
        )}
      </Formik>
    </div>
  );
};

export default CoreSetupNew;
