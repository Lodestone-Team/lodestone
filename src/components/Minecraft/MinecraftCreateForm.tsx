import { MinecraftFlavour } from 'bindings/MinecraftFlavour';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState } from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper } from 'utils/util';
import {
  formId,
  initialValues,
  MinecraftSetupConfigPrimitiveForm,
  validationSchema,
} from './Create/form';
import MinecraftAdvancedForm from './Create/MinecraftAdvancedForm';
import MinecraftBasicForm from './Create/MinecraftBasicForm';
import MinecraftNameForm from './Create/MinecraftNameForm';

function _renderStepContent(step: number) {
  switch (step) {
    case 0:
      return <MinecraftNameForm />;
    case 1:
      return <MinecraftBasicForm />;
    case 2:
      return <MinecraftAdvancedForm />;
    default:
      return 'Unknown step';
  }
}

const steps = ['Name', 'Basic', 'Advanced'];

export default function CreateMinecraftInstance({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [activeStep, setActiveStep] = useState(0);
  const currentValidationSchema = validationSchema[activeStep];
  const formReady = activeStep === steps.length - 1;
  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<MinecraftSetupConfigPrimitiveForm>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  const createInstance = async (value: MinecraftSetupConfigPrimitive) => {
    await axiosWrapper<void>({
      method: 'post',
      url: '/instance/minecraft',
      data: value,
    });
  };

  async function _submitForm(
    values: MinecraftSetupConfigPrimitiveForm,
    actions: FormikHelpers<MinecraftSetupConfigPrimitiveForm>
  ) {
    const parsedValues: MinecraftSetupConfigPrimitive = {
      ...values,
      flavour: values.flavour as MinecraftFlavour,
      cmd_args: values.cmd_args.split(' ').filter((item) => item !== ''),
      auto_start: values.auto_start === 'true',
      restart_on_crash: values.restart_on_crash === 'true',
      start_on_connection: values.start_on_connection === 'true',
    };

    await createInstance(parsedValues);
    actions.setSubmitting(false);

    setActiveStep(activeStep + 1);
    gaEventTracker('Create Instance Complete');
    onComplete();
  }

  function _handleSubmit(
    values: MinecraftSetupConfigPrimitiveForm,
    actions: FormikHelpers<MinecraftSetupConfigPrimitiveForm>
  ) {
    if (formReady) {
      _submitForm(values, actions);
    } else {
      setActiveStep(activeStep + 1);
      actions.setTouched({});
      actions.setSubmitting(false);
    }
  }

  function _handleBack() {
    setActiveStep(activeStep - 1);
  }

  return (
    <div className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-2xl bg-gray-800 px-12 py-24">
      <Formik
        initialValues={initialValues}
        validationSchema={currentValidationSchema}
        onSubmit={_handleSubmit}
        innerRef={formikRef}
        validateOnBlur={false}
        validateOnChange={false}
      >
        {({ isSubmitting }) => (
          <Form
            id={formId}
            className="flex flex-col items-stretch gap-6 text-center"
          >
            {_renderStepContent(activeStep)}

            <div className="flex flex-row justify-between">
              {activeStep !== 0 ? (
                <Button onClick={_handleBack} label="Back" />
              ) : (
                <div></div>
              )}
              <Button
                type="submit"
                label={formReady ? 'Create Instance' : 'Next'}
                loading={isSubmitting}
              />
            </div>
          </Form>
        )}
      </Formik>
    </div>
  );
}
