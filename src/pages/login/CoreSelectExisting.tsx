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
} from '@fortawesome/free-solid-svg-icons';
import SelectCoreField from 'components/Atoms/Form/SelectCoreField';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo } from 'data/SystemInfo';
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
        if(res.data.is_setup === false){
          setPathname('/login/core/first_setup');
        }else{
          setPathname('/login/user/select');
        }
        actions.setSubmitting(false);
      })
      .catch((err) => {
        const errorMessages = errorToString(err);
        actions.setErrors({
          core: {
            address: errorMessages,
            port: errorMessages,
            apiVersion: errorMessages,
            protocol: errorMessages,
          },
        });
        actions.setSubmitting(false);
        return;
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
          <img src="/logo.svg" alt="logo" className="h-9 w-40" />
          <h1 className=" font-title text-2xlarge font-medium tracking-medium text-gray-300">
            Add a new core
          </h1>
        </div>
        <Formik
          initialValues={initialValues}
          validationSchema={validationSchema}
          onSubmit={onSubmit}
        >
          {({ isSubmitting }) => (
            <Form
              id="addCoreForm"
              className="flex flex-col gap-12"
              autoComplete={DISABLE_AUTOFILL}
            >
              <div className="flex h-32 flex-row items-baseline gap-8">
                <SelectCoreField
                  name="core"
                  className="flex-1 grow"
                  options={coreList}
                  label="Select Core"
                />
                <p>OR</p>
                <Button
                  icon={faClone}
                  label="Connect a new core"
                  className="flex-1 grow"
                  onClick={() => setPathname('/login/core/new')}
                />
              </div>
              <div className="flex w-full flex-row justify-end gap-4">
                <Button
                  icon={faArrowLeft}
                  label="Back"
                  onClick={navigateBack}
                />
                <Button
                  type="submit"
                  color="primary"
                  label="Add and Continue"
                  icon={faArrowRight}
                  loading={isSubmitting} //TODO: fix button size changing when loading
                />
              </div>
            </Form>
          )}
        </Formik>
      </div>
    </div>
  );
};

export default CoreSelectExisting;
