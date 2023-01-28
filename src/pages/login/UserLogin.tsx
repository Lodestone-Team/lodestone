import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { LodestoneContext } from 'data/LodestoneContext';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import { useCoreInfo } from 'data/SystemInfo';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { DISABLE_AUTOFILL, isLocalCore } from 'utils/util';
import { loginToCore } from 'utils/apis';
import { tauri } from 'utils/tauriUtil';
import { useDocumentTitle } from 'usehooks-ts';
import WarningAlert from 'components/Atoms/WarningAlert';
export type LoginValues = {
  username: string;
  password: string;
};

const validationSchema = yup.object({
  username: yup.string().required('Username is required'),
  password: yup.string().required('Password is required'),
});

const UserLogin = () => {
  useDocumentTitle('Sign in - Lodestone');
  const { setPathname, navigateBack } = useContext(BrowserLocationContext);
  const { setToken, core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name } = coreInfo ?? {};

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
        if (!response) {
          // this should never end
          actions.setStatus({ error: 'Sign in failed' });
          actions.setSubmitting(false);
          return;
        }
        setToken(response.token, socket);
        setPathname('/');
        actions.setSubmitting(false);
      })
      .catch((error: string) => {
        actions.setStatus({ error: error });
        actions.setSubmitting(false);
      });
  };

  return (
    <div className="flex w-[468px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl bg-gray-850 px-12 py-14 transition-dimensions @container">
      <div className="text flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-fit w-fit" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          Sign in
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
            <div className="grid grid-cols-1 gap-y-14 gap-x-8 ">
              <InputField type="text" name="username" label="Username" />
              <InputField type="password" name="password" label="Password" />
            </div>
            <div className="flex w-full flex-row justify-between gap-4">
              <div className="flex flex-row justify-between gap-4">
                {tauri && isLocalCore(core) ? (
                  <Button
                    type="button"
                    icon={faArrowLeft}
                    label="Switch Account"
                    onClick={navigateBack}
                  />
                ) : (
                  <Button
                    type="button"
                    icon={faArrowLeft}
                    label="Change Core"
                    onClick={() => setPathname('/login/core/select')}
                  />
                )}
              </div>
              <Button
                type="submit"
                intention="primary"
                iconRight={faArrowRight}
                label="Sign in"
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
