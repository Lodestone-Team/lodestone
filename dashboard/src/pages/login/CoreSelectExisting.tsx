import axios from 'axios';
import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import { CoreConnectionInfo, LodestoneContext } from 'data/LodestoneContext';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import {
  faArrowLeft,
  faArrowRight,
  faClone,
  faDownload,
  faPlus,
} from '@fortawesome/free-solid-svg-icons';
import SelectField from 'components/Atoms/Form/SelectField';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo } from 'data/SystemInfo';
import { useDocumentTitle } from 'usehooks-ts';
import WarningAlert from 'components/Atoms/WarningAlert';
type SelectCoreValue = {
  core: CoreConnectionInfo;
};

const validationSchema = yup.object({
  core: yup
    .object({
      address: yup.string().required('Required'),
      port: yup.number().required('Required'),
      apiVersion: yup.string().required('Required'),
      protocol: yup.string().required('Required'),
    })
    .required('Required'),
});

const CoreSelectExisting = () => {
  useDocumentTitle('Select Core - Lodestone');
  const { setPathname, navigateBack } = useContext(BrowserLocationContext);
  const { core, setCore, coreList } = useContext(LodestoneContext);

  const initialValues: SelectCoreValue = {
    core,
  };

  const onSubmit = (
    values: SelectCoreValue,
    actions: FormikHelpers<SelectCoreValue>
  ) => {
    const { core } = values;
    // check if core can be reached
    axios
      .get<CoreInfo>(`/info`, {
        baseURL: `${core.protocol}://${core.address}:${core.port}/api/${core.apiVersion}`,
      })
      .then((res) => {
        if (res.status !== 200)
          throw new Error('Invalid response, setup may be invalid');
        setCore(core);
        if (res.data.is_setup === false) {
          setPathname('/login/core/first_setup');
        } else {
          setPathname('/login/user/select');
        }
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
    <div className="flex w-[640px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl px-12 py-14 @container">
      <div className="flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-8" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          Select Lodestone Core
        </h1>
      </div>
      <Formik
        initialValues={initialValues}
        validationSchema={validationSchema}
        onSubmit={onSubmit}
        validateOnMount={false}
        validateOnChange={false}
        validateOnBlur={false}
      >
        {({ isSubmitting, status }) => (
          <Form
            id="addCoreForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            {status && (
              <WarningAlert>
                <p>
                  <b>{status.error}</b>: Please ensure your fields are filled
                  out correctly.
                </p>
              </WarningAlert>
            )}
            <div className="flex flex-row items-baseline gap-8">
              <SelectField
                name="core"
                className="flex-1"
                options={coreList}
                label="Select Existing Core"
                optionLabel={(option) => {
                  return `${option.address}:${option.port}`;
                }}
              />
              <p className="text-medium font-medium tracking-medium text-white/50">
                OR
              </p>
              <Button
                type="button"
                icon={faPlus}
                label="Connect a new core"
                className="flex-1"
                onClick={() => setPathname('/login/core/new')}
              />
            </div>
            <div className="flex w-full flex-row justify-end gap-4">
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
                label="Continue"
                iconRight={faArrowRight}
                loading={isSubmitting} //TODO: fix button size changing when loading
              />
            </div>
          </Form>
        )}
      </Formik>
    </div>
  );
};

export default CoreSelectExisting;
