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
import { LodestoneContext } from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';

type ConfigNewCoreFormValues = {
  coreName: string;
  domain?: string;
};

const validationSchema = yup.object({
  coreName: yup.string().required('Core name is required'),
  domain: yup.string().max(253, 'Domain cannot be longer than 253 characters'),
});

const CoreConfigNew = () => {
  const { navigateBack, setPathname } = useContext(BrowserLocationContext);
  const { core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name } = coreInfo ?? {};
  const queryClient = useQueryClient();
  const { data: globalSettings, isLoading, error } = useGlobalSettings();

  const initialValues: ConfigNewCoreFormValues = {
    coreName: globalSettings?.core_name || '',
    domain: globalSettings?.domain || '',
  };

  const onSubmit = async (
    values: ConfigNewCoreFormValues,
    actions: FormikHelpers<ConfigNewCoreFormValues>
  ) => {
    // check if core can be reached
    await axiosPutSingleValue('/global_settings/name', values.coreName)
      .then(() => {
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          core_name: values.coreName,
        });
        queryClient.setQueryData(['systeminfo', 'CoreInfo'], {
          ...coreInfo,
          core_name: values.coreName,
        });
      })
      .catch((err) => {
        actions.setSubmitting(false);
        actions.setErrors({ coreName: errorToString(err) });
      });
    await axiosPutSingleValue('/global_settings/domain', values.domain)
      .then(() => {
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          domain: values.domain,
        });
      })
      .catch((err) => {
        actions.setSubmitting(false);
        actions.setErrors({ domain: errorToString(err) });
      });
    actions.setSubmitting(false);
    setPathname('/');
  };

  return (
    <div className="flex w-[468px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl bg-gray-850 px-12 py-14 transition-dimensions @container">
      <div className="text flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-8" />
        <h1 className="font-title text-h1 font-medium tracking-medium text-gray-300">
          Customize your core
        </h1>
        <h2 className="text-h3 font-bold tracking-medium text-white/50">
          {core_name} ({socket})
        </h2>
      </div>
      <Formik
        initialValues={initialValues}
        validationSchema={validationSchema}
        onSubmit={onSubmit}
        validateOnBlur={false}
        validateOnChange={false}
        enableReinitialize={true}
      >
        {({ isSubmitting }) => (
          <Form
            id="configCoreForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            <div className="grid grid-cols-1 gap-y-14 gap-x-8 @lg:grid-cols-2">
              <InputField type="text" name="coreName" label="Core Name" />
              <InputField
                type="text"
                name="domain"
                label="Public Domain/IP (Optional)"
                placeholder="123.123.123.123"
              />
            </div>
            <div className="flex w-full flex-row justify-end gap-4">
              <Button
                type="submit"
                color="primary"
                label="Continue"
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

export default CoreConfigNew;
