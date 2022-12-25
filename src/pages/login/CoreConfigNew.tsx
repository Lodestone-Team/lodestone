import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import {
  axiosPutSingleValue,
  DISABLE_AUTOFILL,
  errorToString,
} from 'utils/util';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { useQueryClient } from '@tanstack/react-query';
import { useGlobalSettings } from 'data/GlobalSettings';

type ConfigNewCoreFormValues = {
  coreName: string;
};

const validationSchema = yup.object({
  coreName: yup.string().required('Core name is required'),
});

const CoreConfigNew = () => {
  const { navigateBack, setPathname } = useContext(BrowserLocationContext);
  const queryClient = useQueryClient();
  const { data: globalSettings, isLoading, error } = useGlobalSettings();

  const initialValues: ConfigNewCoreFormValues = {
    coreName: globalSettings?.core_name || '',
  };

  const onSubmit = async (
    values: ConfigNewCoreFormValues,
    actions: FormikHelpers<ConfigNewCoreFormValues>
  ) => {
    // check if core can be reached
    axiosPutSingleValue('/global_settings/name', values.coreName)
      .then(() => {
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          core_name: values.coreName,
        });
        setPathname('/login/user/select');//TODO: auto login
      })
      .catch((err) => {
        actions.setSubmitting(false);
        actions.setErrors({ coreName: errorToString(err) });
      });
    // TODO: maybe update global settings here
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
          <img src="/logo.svg" alt="logo" className="h-9 w-40" />
          <h1 className=" font-title text-2xlarge font-medium tracking-medium text-gray-300">
            Customize your core
          </h1>
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
              id="configCoreForm"
              className="flex flex-col gap-12"
              autoComplete={DISABLE_AUTOFILL}
            >
              <div className="grid h-32 grid-cols-1 gap-y-14 gap-x-8 @lg:grid-cols-2">
                <InputField type="text" name="coreName" label="Core Name" />
              </div>
              <div className="flex w-full flex-row justify-end gap-4">
                <Button
                  iconRight={faArrowLeft}
                  label="Back"
                  onClick={navigateBack}
                />
                <Button
                  type="submit"
                  color="primary"
                  label="Submit"
                  iconRight={faArrowRight}
                  loading={isSubmitting}
                />
              </div>
            </Form>
          )}
        </Formik>
      </div>
    </div>
  );
};

export default CoreConfigNew;
