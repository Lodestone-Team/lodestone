import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState, useEffect } from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper } from 'utils/util';
import {
  ConfigurableValue,
  formId,
  ManifestValue,
  ConfigurableManifest,
  SectionManifestValue,
} from './Create/form';
import { generateValidationSchema, generateInitialValues } from './Create/form';
import { createForm } from './Create/FormCreation';
import GameTypeSelectForm from './Create/GameTypeSelectForm';
import { SetupInstanceManifest } from 'data/InstanceGameTypes';
import { HandlerGameType } from 'bindings/HandlerGameType';
import Spinner from 'components/DashboardLayout/Spinner';
import clsx from 'clsx';

function _renderStepContent(
  step: number,
  gameType: HandlerGameType,
  setGameType: (gameType: HandlerGameType) => void,
  setupManifest?: ConfigurableManifest | null
) {
  if (!setupManifest) {
    return null;
  }
  const forms = Object.keys(setupManifest['setting_sections']).map(
    (key: string) => {
      // const section:SectionManifest = setupManifest['setting_sections'][key];
      return createForm(setupManifest['setting_sections'][key]);
    }
  );

  return (
    <div className="h-full">
      {step == 0 ? (
        <GameTypeSelectForm gameType={gameType} setGameType={setGameType} />
      ) : (
        forms[step - 1]
      )}
    </div>
  );
}

export default function CreateMinecraftInstance({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [activeStep, setActiveStep] = useState(0);
  const [gameType, setGameType] = useState<HandlerGameType>(
    'MinecraftJavaVanilla'
  );
  const {
    data: setup_manifest,
    isLoading,
    error,
  } = SetupInstanceManifest(gameType);

  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue | null>>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  useEffect(() => {
    if (!isLoading && !error) {
      setSetupManifest(setup_manifest);
    }
  }, [gameType, isLoading, setup_manifest, error]);

  const [setupManifest, setSetupManifest] =
    useState<ConfigurableManifest | null>(null);

  if (setupManifest === null) return <Spinner />;

  const initialValue: Record<string, ConfigurableValue | null> =
    generateInitialValues(setupManifest['setting_sections']);
  const validationSchema = generateValidationSchema(setupManifest);
  const currentValidationSchema = validationSchema[activeStep];
  const steps = [
    'Select Game',
    Object.keys(setupManifest['setting_sections']).map(
      (sectionId) => setupManifest['setting_sections'][sectionId]['name']
    ),
  ].flat();
  const formReady = activeStep === steps.length - 1;
  const createInstance = async (value: ManifestValue) => {
    await axiosWrapper<void>({
      method: 'post',
      url: `/instance/create/${gameType}`,
      headers: { 'Content-Type': 'application/json' },
      data: JSON.stringify(value),
    });
  };

  async function _submitForm(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    const sectionValues: Record<string, SectionManifestValue> = {};
    for (let i = 1; i < steps.length - 1; i++) {
      const structure = getSectionValidationStructure(values, i);
      sectionValues[structure[1]] = structure[0];
    }

    const parsedValues: ManifestValue = {
      auto_start: values.auto_start?.value as boolean,
      restart_on_crash: values.restart_on_crash?.value as boolean,
      start_on_connection: values.start_on_connection?.value as boolean,
      setting_sections: sectionValues,
    };

    await createInstance(parsedValues);
    actions.setSubmitting(false);

    setActiveStep(activeStep + 1);
    gaEventTracker('Create Instance Complete');
    onComplete();
  }

  function getSectionValidationStructure(
    values: Record<string, ConfigurableValue | null>,
    step: number
  ): [SectionManifestValue, string] {
    if (!setupManifest || step == 0) return [{ settings: {} }, ''];
    const sectionKeys = Object.keys(setupManifest['setting_sections']);
    const settingKeys = Object.keys(
      setupManifest['setting_sections'][sectionKeys[step - 1]]['settings']
    );
    const sectionValidation: SectionManifestValue = { settings: {} };
    for (const key of settingKeys) {
      sectionValidation['settings'][key] = { value: values[key] };
    }
    return [sectionValidation, sectionKeys[step - 1]];
  }

  async function _sectionValidation(
    values: Record<string, ConfigurableValue | null>,
    step: number
  ) {
    const structure = getSectionValidationStructure(values, step);
    if (!structure[1]) return; //if string is empty
    const sectionValidation = structure[0];
    const sectionKey = structure[1];

    const result = await axiosWrapper<void>({
      method: 'put',
      url: `/setup_manifest/${gameType}/${sectionKey}`,
      headers: { 'Content-Type': 'application/json' },
      data: JSON.stringify(sectionValidation),
    });

    console.log(result);
  }

  function _handleSubmit(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    if (formReady) {
      _submitForm(values, actions);
    } else {
      _sectionValidation(values, activeStep);
      setActiveStep(activeStep + 1);
      actions.setTouched({});
      actions.setSubmitting(false);
    }
  }

  function _handleBack() {
    setActiveStep(activeStep - 1);
  }

  return (
    <Formik
      initialValues={initialValue}
      validationSchema={currentValidationSchema}
      onSubmit={_handleSubmit}
      innerRef={formikRef}
      validateOnBlur={false}
      validateOnChange={false}
    >
      {({ isSubmitting }) => (
        <Form
          id={formId}
          className="flex h-[560px] w-[812px] items-stretch rounded-2xl bg-gray-850"
        >
          <div className="flex w-[180px] border-r border-gray-700 ">
            <div className="mt-9">
              {steps.map((section, i) => (
                <div key={i} className="flex items-center">
                  <div
                    className={clsx(
                      'px-4 py-2 text-left font-sans text-medium font-medium leading-5 tracking-medium text-white/50 ',
                      activeStep === i && 'font-extrabold text-white'
                    )}
                  >
                    {section}
                  </div>
                </div>
              ))}
            </div>
          </div>
          <div className="relative flex h-full w-[632px] grow flex-col items-stretch overflow-auto p-9 text-center">
            <div className="relative">
              {_renderStepContent(
                activeStep,
                gameType,
                setGameType,
                setupManifest
              )}
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
            </div>
          </div>
        </Form>
      )}
    </Formik>
  );
}
