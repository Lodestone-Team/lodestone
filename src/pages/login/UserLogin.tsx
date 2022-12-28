import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { LodestoneContext } from 'data/LodestoneContext';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import { useCoreInfo } from 'data/SystemInfo';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { DISABLE_AUTOFILL, loginToCore } from 'utils/util';

export type LoginValues = {
  username: string;
  password: string;
};

const validationSchema = yup.object({
  username: yup.string().required('Username is required'),
  password: yup.string().required('Password is required'),
});

const UserLogin = () => {
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
          actions.setErrors({ password: 'Sign in failed' });
          actions.setSubmitting(false);
          return;
        }
        setToken(response.token, socket);
        setPathname('/');
        actions.setSubmitting(false);
      })
      .catch((error: string) => {
        actions.setErrors({ password: error });
        actions.setSubmitting(false);
      });
  };

  return (
    <div className="flex w-[468px] max-w-full flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-850 px-12 py-12 transition-dimensions @container">
      <div className="text flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-9 w-40" />
        <h1 className="font-title text-2xlarge font-medium-semi-bold tracking-medium text-gray-300">
          Sign in
        </h1>
        <h2 className="text-medium font-semibold tracking-medium text-white/50">
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
        {({ isSubmitting }) => (
          <Form
            id="loginForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            <div className="grid grid-cols-1 gap-y-14 gap-x-8 ">
              <InputField type="text" name="username" label="Username" />
              <InputField type="password" name="password" label="Password" />
            </div>
            <div className="flex w-full flex-row justify-between gap-4">
              <Button icon={faArrowLeft} label="Back" onClick={navigateBack} />
              <Button
                type="submit"
                color="primary"
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
