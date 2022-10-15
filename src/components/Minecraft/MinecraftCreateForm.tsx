import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers } from 'formik';
import { useState } from 'react';
import { axiosWrapper } from 'utils/util';
import { formId, initialValues, validationSchema } from './Create/form';
import MinecraftAdvancedForm from './Create/MinecraftAdvancedForm';
import MinecraftBasicForm from './Create/MinecraftBasicForm';
import MinecraftNameForm from './Create/MinecraftNameForm';

function _renderStepContent(step: number, toggleAdvanced: () => void) {
  switch (step) {
    case 0:
      return <MinecraftNameForm />;
    case 1:
      return <MinecraftBasicForm toggleAdvanced={toggleAdvanced} />;
    case 2:
      return <MinecraftAdvancedForm toggleAdvanced={toggleAdvanced} />;
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
  const formReady = activeStep === 1 || activeStep === 2;

  const createInstance = async (value: MinecraftSetupConfigPrimitive) => {
    await axiosWrapper<void>({
      method: 'post',
      url: '/instance/minecraft',
      data: value,
    });
  };

  async function _submitForm(
    values: MinecraftSetupConfigPrimitive,
    actions: FormikHelpers<MinecraftSetupConfigPrimitive>
  ) {
    await createInstance(values);
    actions.setSubmitting(false);

    setActiveStep(activeStep + 1);
    onComplete();
  }

  function _handleSubmit(
    values: MinecraftSetupConfigPrimitive,
    actions: FormikHelpers<MinecraftSetupConfigPrimitive>
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
  
  function toggleAdvanced() {
    setActiveStep(activeStep === 1 ? 2 : 1);
  }

  return (
    <div className="flex w-[500px] flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-900 px-12 py-24">
      <Formik
        initialValues={initialValues}
        validationSchema={currentValidationSchema}
        onSubmit={_handleSubmit}
      >
        {({ isSubmitting }) => (
          <Form
            id={formId}
            className="flex flex-col items-stretch gap-6 text-center"
          >
            {_renderStepContent(activeStep, toggleAdvanced)}

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
