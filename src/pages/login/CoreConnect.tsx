import axios from 'axios';
import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { DISABLE_AUTOFILL, errorToString, LODESTONE_PORT } from 'utils/util';
import { CoreConnectionInfo, LodestoneContext } from 'data/LodestoneContext';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import SelectField from 'components/Atoms/Form/SelectField';
import {
  faArrowLeft,
  faArrowRight,
  faDownload,
} from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo } from 'data/SystemInfo';
import { useDocumentTitle } from 'usehooks-ts';

const validationSchema = yup.object({
  address: yup.string().required('Required'),
  port: yup.number().required('Required'),
  apiVersion: yup.string().required('Required'),
  protocol: yup.string().required('Required'),
});

const CoreConnect = () => {
  useDocumentTitle('Connect to Core - Lodestone');
  const { navigateBack, setPathname } = useContext(BrowserLocationContext);
  const { setCore, addCore } = useContext(LodestoneContext);

  const initialValues: CoreConnectionInfo = {
    address: '',
    port: LODESTONE_PORT.toString(),
    apiVersion: 'v1',
    protocol: 'http', //change to https when supported
  };

  const onSubmit = (
    values: CoreConnectionInfo,
    actions: FormikHelpers<CoreConnectionInfo>
  ) => {
    // check if core can be reached
    axios
      .get<CoreInfo>(`/info`, {
        baseURL: `${values.protocol}://${values.address}:${values.port}/api/${values.apiVersion}`,
      })
      .then((res) => {
        if (res.status !== 200)
          throw new Error('Invalid response, setup may be invalid');
        setCore(values);
        addCore(values);
        if (res.data.is_setup === false) {
          setPathname('/login/core/first_setup');
        } else {
          setPathname('/login/user/select');
        }
        actions.setSubmitting(false);
      })
      .catch((err) => {
        const errorMessages = errorToString(err);
        actions.setErrors({ address: errorMessages }); //TODO: put the error in a better place, it's not just an address problem
        actions.setSubmitting(false);
        return;
      });
  };

  return (
    <div className="flex w-[768px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl px-12 py-14 @container">
      <div className="text flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-8" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          Add a new core
        </h1>
        <h2 className="text-medium font-medium tracking-medium text-white/75">
          You may need to adjust your network and browser settings.{' '}
          <a
            href="https://github.com/Lodestone-Team/dashboard/wiki/Known-Issues#networking"
            target="_blank"
            rel="noreferrer"
            className="text-blue-200 underline hover:text-blue-300"
          >
            Learn more
          </a>
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
            id="addCoreForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            <div className="grid grid-cols-1 gap-y-14 gap-x-8 @lg:grid-cols-2">
              <SelectField
                name="apiVersion"
                className="grow"
                options={['v1']}
                label="API Version"
              />
              <SelectField
                name="protocol"
                className="grow"
                options={['http', 'https']}
                label="Protocol"
              />
              <InputField type="number" name="port" label="Port" />
              <InputField
                type="text"
                name="address"
                label="IP Address/Domain"
                placeholder='e.g. 123.123.123.123 or "myserver.com"'
              />
            </div>
            <div className="flex w-full flex-row justify-between gap-4">
              <Button
                type="button"
                icon={faArrowLeft}
                label="Back"
                onClick={navigateBack}
              />
              <div className="flex flex-row gap-4">
                <Button
                  type="button"
                  iconRight={faDownload}
                  label="Download Lodestone Core"
                  onClick={() => {
                    window.open(
                      'https://github.com/Lodestone-Team/dashboard/releases/',
                      '_self'
                    );
                  }}
                />
                <Button
                  type="submit"
                  intention="primary"
                  label="Add and Continue"
                  iconRight={faArrowRight}
                  loading={isSubmitting} //TODO: fix button size changing when loading
                />
              </div>
            </div>
          </Form>
        )}
      </Formik>
    </div>
  );
};

export default CoreConnect;
