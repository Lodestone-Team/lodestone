import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState, useEffect, useMemo } from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper, catchAsyncToString } from 'utils/util';
import {
  ConfigurableValue,
  formId,
  ManifestValue,
  ConfigurableManifest,
  SectionManifestValue,
} from './Create/form';
import { generateValidationSchema, generateInitialValues } from './Create/form';
import { FormFromManifest } from './Create/FormFromManifest';
import GameTypeSelectForm from './Create/GameTypeSelectForm';
import { SetupInstanceManifest } from 'data/InstanceGameTypes';
import { HandlerGameType } from 'bindings/HandlerGameType';
import Spinner from 'components/DashboardLayout/Spinner';
import WarningAlert from 'components/Atoms/WarningAlert';
import clsx from 'clsx';

export default function CreateGameInstance({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [activeStep, setActiveStep] = useState(0);
  const [gameType, setGameType] = useState<HandlerGameType>(
    'MinecraftJavaVanilla'
  );
  const {
    data: setupManifest,
    isLoading,
    error,
  } = SetupInstanceManifest(gameType);

  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue | null>>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  const initialValue: Record<string, ConfigurableValue | null> | null = useMemo(
    () =>
      setupManifest
        ? generateInitialValues(setupManifest.setting_sections)
        : null,
    [setupManifest]
  );
  const validationSchema = useMemo(
    () => (setupManifest ? generateValidationSchema(setupManifest) : null),
    [setupManifest]
  );
  if (!setupManifest) return <Spinner />;
  if (!initialValue) return <Spinner />;
  if (!validationSchema) return <Spinner />;
  const currentValidationSchema = validationSchema[activeStep];

  const sectionNames = [
    'Select Game',
    Object.keys(setupManifest.setting_sections).map(
      (sectionId) => setupManifest.setting_sections[sectionId].name
    ),
  ].flat();
  const formReady = activeStep === sectionNames.length - 1;
  const createInstance = async (value: ManifestValue) => {
    await axiosWrapper<void>({
      method: 'post',
      url: `/instance/create/${gameType}`,
      headers: { 'Content-Type': 'application/json' },
      data: JSON.stringify(value),
    });
  };

  async function submitForm(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    const sectionValues: Record<string, SectionManifestValue> = {};
    for (let i = 1; i < sectionNames.length - 1; i++) {
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

    gaEventTracker('Create Instance Complete');
    onComplete();
  }

  function getSectionValidationStructure(
    values: Record<string, ConfigurableValue | null>,
    step: number
  ): [SectionManifestValue, string] {
    if (!setupManifest || step == 0) return [{ settings: {} }, ''];
    const sectionKeys = Object.keys(setupManifest.setting_sections);
    const settingKeys = Object.keys(
      setupManifest.setting_sections[sectionKeys[step - 1]]['settings']
    );
    const sectionValidation: SectionManifestValue = { settings: {} };
    for (const key of settingKeys) {
      sectionValidation['settings'][key] = { value: values[key] };
    }
    return [sectionValidation, sectionKeys[step - 1]];
  }

  /**
   * @returns: Error message if there is error, empty string otherwise
   */
  async function validateSection(
    values: Record<string, ConfigurableValue | null>,
    step: number
  ) {
    const structure = getSectionValidationStructure(values, step);
    if (!structure[1]) return; //if string is empty
    const sectionValidation = structure[0];
    const sectionKey = structure[1];

    return catchAsyncToString(
      axiosWrapper<void>({
        method: 'put',
        url: `/setup_manifest/${gameType}/${sectionKey}`,
        headers: { 'Content-Type': 'application/json' },
        data: JSON.stringify(sectionValidation),
      })
    );
  }

  async function handleSubmit(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    if (formReady) {
      submitForm(values, actions);
    } else {
      const errorMessage = await validateSection(values, activeStep);
      if (errorMessage) {
        actions.setStatus({ error: errorMessage });
      } else {
        actions.setStatus(null);
        setActiveStep(activeStep + 1);
        actions.setTouched({});
      }
      actions.setSubmitting(false);
    }
  }

  function handleBack() {
    setActiveStep(activeStep - 1);
  }

  console.log('setupManifest.setting_sections', setupManifest.setting_sections);

  return (
    <Formik
      initialValues={initialValue}
      validationSchema={currentValidationSchema}
      onSubmit={handleSubmit}
      innerRef={formikRef}
      validateOnBlur={false}
      validateOnChange={false}
    >
      {({ isSubmitting, status }) => (
        <Form
          id={formId}
          className="flex h-fit min-h-[560px] w-[812px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg"
        >
          <div className="w-[180px] border-r border-gray-700 pt-9 ">
            {sectionNames.map((section, i) => (
              <div
                key={i}
                className={clsx(
                  'px-4 py-2 text-left font-sans text-medium font-medium leading-5 tracking-medium text-white/50 ',
                  activeStep === i && 'font-extrabold text-white'
                )}
              >
                {section}
              </div>
            ))}
          </div>
          <div className="flex w-[632px] flex-col p-9">
            {activeStep == 0 ? (
              <GameTypeSelectForm
                gameType={gameType}
                setGameType={setGameType}
              />
            ) : (
              <FormFromManifest
                section={
                  Object.values(setupManifest.setting_sections)[activeStep - 1]
                }
              >
                {status && (
                  <WarningAlert>
                    <p>
                      Please ensure your fields are filled out correctly
                      <br />
                      {status.error}
                    </p>
                  </WarningAlert>
                )}
              </FormFromManifest>
            )}
            <div className="flex flex-row justify-between pt-9">
              {activeStep !== 0 ? (
                <Button onClick={handleBack} label="Back" />
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
        </Form>
      )}
    </Formik>
  );
}
