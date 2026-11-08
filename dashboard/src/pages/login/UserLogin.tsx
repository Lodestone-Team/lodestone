import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { LodestoneContext } from 'data/LodestoneContext';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import { useCoreInfo } from 'data/SystemInfo';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { DISABLE_AUTOFILL, errorToString, isLocalCore } from 'utils/util';
import { LoginValues, loginToCore } from 'utils/apis';
import { tauri } from 'utils/tauriUtil';
import { useDocumentTitle } from 'usehooks-ts';
import WarningAlert from 'components/Atoms/WarningAlert';
import useAnalyticsEventTracker from 'utils/hooks';
import Logo from 'components/Atoms/Logo';
import { t as translate, useT } from 'i18n';
export type { LoginValues };

const validationSchema = yup.object({
  username: yup
    .string()
    .required(() => translate('shell.login.usernameRequired')),
  password: yup
    .string()
    .required(() => translate('shell.login.passwordRequired')),
});

const UserLogin = () => {
  const t = useT();
  useDocumentTitle(t('common.pageTitle', { page: t('shell.login.signIn') }));
  const { setPathname, navigateBack } = useContext(BrowserLocationContext);
  const { setToken, core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name } = coreInfo ?? {};
  const gaEventTracker = useAnalyticsEventTracker('User Login');

  const initialValues: LoginValues = {
    username: '',
    password: '',
  };

  const onSubmit = (
    values: LoginValues,
    actions: FormikHelpers<LoginValues>
  ) => {
    // login using basic auth
    loginToCore(values)
      .then((response) => {
        setToken(response.token, socket);
        setPathname('/');
        gaEventTracker('Logged in', 'User');
        actions.setSubmitting(false);
      })
      .catch((error: unknown) => {
        actions.setStatus({ error: errorToString(error) });
        actions.setSubmitting(false);
      });
  };

  return (
    <div className="@container flex w-[468px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl px-12 py-14 transition-dimensions">
      <div className="flex flex-col items-start">
        <Logo alt="Lodestone" className="h-8" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          {t('shell.login.signIn')}
        </h1>
        <h2 className="text-medium font-medium tracking-medium text-white/50">
          {core_name} ({socket})
        </h2>
      </div>
      <Formik
        initialValues={initialValues}
        validationSchema={validationSchema}
        onSubmit={onSubmit}
        validateOnBlur={false}
        validateOnChange={false}
      >
        {({ isSubmitting, status }) => (
          <Form
            id="loginForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            {status && (
              <WarningAlert>
                <p>{status.error}</p>
              </WarningAlert>
            )}
            <div className="grid grid-cols-1 gap-x-8 gap-y-14">
              <InputField
                type="text"
                name="username"
                label={t('shell.login.username')}
              />
              <InputField
                type="password"
                name="password"
                label={t('shell.login.password')}
              />
            </div>
            <div className="flex w-full flex-row justify-between gap-4">
              <div className="flex flex-row justify-between gap-4">
                {tauri && isLocalCore(core) ? (
                  <Button
                    type="button"
                    icon={faArrowLeft}
                    label={t('shell.userLogin.switchAccount')}
                    onClick={navigateBack}
                  />
                ) : (
                  <Button
                    type="button"
                    icon={faArrowLeft}
                    label={t('shell.login.changeCore')}
                    onClick={() => setPathname('/login/core/select')}
                  />
                )}
              </div>
              <Button
                type="submit"
                intention="primary"
                iconRight={faArrowRight}
                label={t('shell.login.signIn')}
                loading={isSubmitting}
              />
            </div>
          </Form>
        )}
      </Formik>
    </div>
  );
};

export default UserLogin;
