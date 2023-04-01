import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState, useEffect, useMemo } from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper } from 'utils/util';
import {
  ConfigurableValue,
  formId,
  ManifestValue,
  ConfigurableManifest,
  SectionManifestValue,
  SetupManifest,
  SetupValue,
} from './Create/form';
import { generateValidationSchema, generateInitialValues } from './Create/form';
import { createForm } from './Create/FormCreation';
import GameTypeSelectForm from './Create/GameTypeSelectForm';
import {
  SetupGenericInstanceManifest,
  SetupInstanceManifest,
} from 'data/InstanceGameTypes';
import { HandlerGameType } from 'bindings/HandlerGameType';
import Spinner from 'components/DashboardLayout/Spinner';
import clsx from 'clsx';
import * as yup from 'yup';

export type GenericHandlerGameType = 'Generic' | HandlerGameType;

function _renderStepContent(
  step: number,
  gameType: GenericHandlerGameType,
  setGameType: (gameType: GenericHandlerGameType) => void,
  setupManifest?: SetupManifest | null
) {
  const forms = useMemo(() => {
    if (!setupManifest) return [];
    return Object.keys(setupManifest['setting_sections']).map((key: string) => {
      return createForm(setupManifest['setting_sections'][key]);
    });
  }, [setupManifest]);

  if (forms.length == 0 && step != 0) return <Spinner />;
  return (
    <>
      {step == 0 ? (
        <GameTypeSelectForm
          selectedGameType={gameType}
          setGameType={setGameType}
        />
      ) : (
        forms[step - 1]
      )}
    </>
  );
}

export default function CreateGameInstance({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [activeStep, setActiveStep] = useState(0);
  const [gameType, setGameType] = useState<GenericHandlerGameType>(
    'MinecraftJavaVanilla'
  );
  // const ready = activeStep !== 0;
  // console.log(ready);
  const {
    data: setup_manifest,
    isLoading,
    error,
  } = gameType === 'Generic'
    ? SetupGenericInstanceManifest(gameType, '', false)
    : SetupInstanceManifest(gameType as HandlerGameType);

  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue | null>>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  useEffect(() => {
    console.log(gameType);
    console.log(setup_manifest);
    if (!isLoading && !error) {
      setInitialValues(
        generateInitialValues(setup_manifest['setting_sections'])
      );
      setValidationSchema(generateValidationSchema(setup_manifest));
      setSetupManifest(setup_manifest);
    }
  }, [gameType, isLoading, setup_manifest, error]);

  const [setupManifest, setSetupManifest] = useState<SetupManifest | null>(
    null
  );

  const [initialValues, setInitialValues] = useState<
    Record<string, ConfigurableValue | null>
  >({});
  const [validationSchema, setValidationSchema] = useState<any[]>([
    yup.object().shape({}),
  ]);

  if (setupManifest === null && activeStep !== 0) return <Spinner />;

  // console.log(setupManifest);
  // const initialValues: Record<string, ConfigurableValue | null> = setupManifest
  //   ? generateInitialValues(setupManifest['setting_sections'])
  //   : {};
  // const validationSchema: any[] = setupManifest
  //   ? generateValidationSchema(setupManifest)
  //   : [yup.object().shape({})];
  const currentValidationSchema = validationSchema[activeStep];
  const steps = [
    'Select Game',
    'Basic Settings',
    'Instance Settings',
    'Auto Settings',
    // Object.keys(setupManifest['setting_sections']).map(
    //   (sectionId) => setupManifest['setting_sections'][sectionId]['name']
    // ),
  ].flat();
  const formReady = activeStep === steps.length - 1;
  const createInstance = async (value: SetupValue) => {
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

    const parsedValues: SetupValue = {
      name: values.name?.value as string,
      description: values.description?.value as string,
      auto_start: values.auto_start?.value as boolean,
      restart_on_crash: values.restart_on_crash?.value as boolean,
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

    // const result = await axiosWrapper<void>({
    //   method: 'put',
    //   url: `/setup_manifest/${gameType}/${sectionKey}`,
    //   headers: { 'Content-Type': 'application/json' },
    //   data: JSON.stringify(sectionValidation),
    // });
  }

  // function _getSetupManifest(
  //   values: Record<string, ConfigurableValue | null>,
  //   actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  // ) {
  //   if (!isLoading && !error && setupManifest) {
  //     console.log(generateInitialValues(setupManifest['setting_sections']));
  //     setInitialValues(
  //       generateInitialValues(setupManifest['setting_sections'])
  //     );
  //     setValidationSchema(generateValidationSchema(setupManifest));
  //     setActiveStep(activeStep + 1);

  //     actions.setTouched({});
  //     actions.setSubmitting(false);
  //   }
  // }

  function _handleSubmit(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    if (formReady) {
      _submitForm(values, actions);
    } else {
      console.log(initialValues);
      _sectionValidation(values, activeStep);
      setActiveStep(activeStep + 1);
      actions.setValues(initialValues);
      actions.setTouched({});
      actions.setSubmitting(false);
    }
  }

  function _handleBack() {
    setActiveStep(activeStep - 1);
  }

  return (
    <Formik
      initialValues={initialValues}
      validationSchema={currentValidationSchema}
      onSubmit={_handleSubmit}
      innerRef={formikRef}
      validateOnBlur={false}
      validateOnChange={false}
    >
      {({ isSubmitting, setValues }) => (
        <Form
          id={formId}
          className="flex max-h-[700px] min-h-[560px] w-[812px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg"
        >
          <div className="w-[180px] border-r border-gray-700 pt-9 ">
            {steps.map((section, i) => (
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
            {_renderStepContent(
              activeStep,
              gameType,
              setGameType,
              setupManifest
            )}
            <div className="flex flex-row justify-between pt-9">
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
        </Form>
      )}
    </Formik>
  );
}
