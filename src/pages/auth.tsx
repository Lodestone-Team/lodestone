import axios, { AxiosError } from 'axios';
import Button from 'components/Atoms/Button';
import { useRouter } from 'next/router';
import { useContext, useEffect } from 'react';
import { pushKeepQuery } from 'utils/util';
import { NextPageWithLayout } from './_app';
import Link from 'next/link';
import { LodestoneContext } from 'data/LodestoneContext';
import { ClientError } from 'bindings/ClientError';
import { LoginReply } from 'bindings/LoginReply';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';

type LoginValues = {
  username: string;
  password: string;
};

const initialValues: LoginValues = {
  username: '',
  password: '',
};

const validationSchema = yup.object({
  username: yup.string().required('Username is required'),
  password: yup.string().required('Password is required'),
});

const Auth: NextPageWithLayout = () => {
  const router = useRouter();
  const { token, setToken, address, port } = useContext(LodestoneContext);

  useEffect(() => {
    if (token) {
      alert('Logged in!');
      pushKeepQuery(router, '/');
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

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
        setToken(response.data.token);
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
    <div className="flex h-screen flex-col items-center justify-center bg-gray-800">
      <div className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-900 px-12 py-24">
        <div className="flex flex-col items-center gap-6 text-center">
          <img src="/logo.svg" alt="logo" className="w-40" />
          <h1 className="text-larger font-bold tracking-tight text-gray-300">
            Welcome Back!
          </h1>
          <p>
            Connect to Lodestone core at {`${address}:${port}`}
            {/* <br />
            Don&apos;t have Lodestone?{' '}
            <Link href="/get-started">
              <span className="text-green-accent hover:cursor-pointer hover:text-green">
                Get started here
              </span>
            </Link> */}
          </p>
        </div>
        <Formik
          initialValues={initialValues}
          validationSchema={validationSchema}
          onSubmit={onSubmit}
        >
          {({ isSubmitting }) => (
            <Form
              id="loginForm"
              className="flex flex-col gap-12"
              autoComplete="nope"
            >
              <div className="flex flex-col gap-y-12">
                <InputField type="text" name="username" label="Username" />
                <InputField type="password" name="password" label="Password" />
              </div>
              <Button type="submit" label="Login" loading={isSubmitting} />
            </Form>
          )}
        </Formik>
      </div>
    </div>
  );
};

export default Auth;
