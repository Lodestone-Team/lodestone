import axios, { AxiosError } from 'axios';
import Button from 'components/Atoms/Button';
import { useRouter } from 'next/router';
import { useContext, useEffect } from 'react';
import { pushKeepQuery } from 'utils/util';
import { NextPageWithLayout } from '../_app';
import Link from 'next/link';
import { LodestoneContext } from 'data/LodestoneContext';
import { ClientError } from 'bindings/ClientError';
import { LoginReply } from 'bindings/LoginReply';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import ComboField from 'components/Atoms/Form/ComboField';
import { useCoreInfo } from 'data/SystemInfo';
import NoSSR from 'react-no-ssr';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';

type LoginValues = {
  username: string;
  password: string;
};

const validationSchema = yup.object({
  username: yup.string().required('Username is required'),
  password: yup.string().required('Password is required'),
});

const Auth: NextPageWithLayout = () => {
  const router = useRouter();
  const { token, setToken, tokens, isReady, core } =
    useContext(LodestoneContext);
  const { protocol, apiVersion, address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name } = coreInfo ?? {};

  const initialValues: LoginValues = {
    username: '',
    password: '',
  };

  console.log(initialValues);

  const onSubmit = (
    values: LoginValues,
    actions: FormikHelpers<LoginValues>
  ) => {
    // login using basic auth
    axios
      .post<LoginReply>(
        '/user/login',
        {},
        {
          auth: values,
        }
      )
      .then((response) => {
        // set the token cookie
        setToken(response.data.token, socket);
        // redirect to the home page, and set the query
        pushKeepQuery(router, '/');
      })
      .catch((error: AxiosError<ClientError>) => {
        if (axios.isAxiosError(error) && error.response) {
          if (
            error.response.status === 401 ||
            error.response.status === 403 ||
            error.response.status === 500
          ) {
            alert(
              `Error: ${error.response.data.inner}: ${error.response.data.detail}`
            );
          }
        } else {
          alert(`Login failed: ${error.message}`);
        }
      })
      .finally(() => {
        actions.setSubmitting(false);
      });
  };

  return (
    <div
      className="flex h-screen flex-col items-center justify-center p-8"
      style={{
        background: "url('/login_background.svg')",
        backgroundSize: 'cover',
      }}
    >
      <div className="flex w-[768px] max-w-full flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-850 px-14 py-20 @container">
        <div className="text flex flex-col items-start">
          <NoSSR>
            <h1 className=" font-title text-2xlarge font-medium tracking-medium text-gray-300">
              Sign-in to {core_name ?? '...'}
            </h1>
            <h2 className="text-medium font-medium tracking-medium text-gray-300 h-9">
              Base URL: {socket}
            </h2>
          </NoSSR>
        </div>
        {isReady ? (
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
                autoComplete="nope"
              >
                <div className="grid grid-cols-1 gap-y-14 gap-x-8 @lg:grid-cols-2 h-32">
                  <InputField type="text" name="username" label="Username" />
                  <InputField
                    type="password"
                    name="password"
                    label="Password"
                  />
                </div>
                <div className="flex w-full flex-row justify-end gap-4">
                  <Button
                    icon={faArrowLeft}
                    label="Change Core"
                    onClick={() => pushKeepQuery(router, '/login/core/select')}
                  />
                  <Button
                    type="submit"
                    color="primary"
                    icon={faArrowRight}
                    label="Login"
                    loading={isSubmitting}
                  />
                </div>
              </Form>
            )}
          </Formik>
        ) : (
          <p>Loading...</p>
        )}
      </div>
    </div>
  );
};

export default Auth;
