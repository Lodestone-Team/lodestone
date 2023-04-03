import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import {
  useRef,
  useState,
  useEffect,
  useMemo,
  Dispatch,
  SetStateAction,
} from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper } from 'utils/util';
import {
  autoSettingPageObject,
  basicSettingsPageObject,
  ConfigurableValue,
  formId,
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
export type FormPage = {
  name: string;
  description: string;
  page: SetupManifest;
};
function _renderStepContent(
  step: number,
  gameType: GenericHandlerGameType,
  setGameType: (gameType: GenericHandlerGameType) => void,
  urlValid: boolean,
  setUrlValid: Dispatch<SetStateAction<boolean>>,
  setUrl: (url: string) => void,
  setupManifest?: SetupManifest | null
) {
  const forms = useMemo(() => {
    if (!setupManifest) return [];
    // console.log(Object.keys(setupManifest));
    const pages: FormPage[] = [
      {
        name: 'Basic Settings',
        description: 'Basic settings for your server.',
        page: { setting_sections: { section_1: basicSettingsPageObject } },
      },
      {
        name: 'Instance Settings',
        description: 'Configure your server.',
        page: setupManifest,
      },
      {
        name: 'Auto Settings',
        description: 'Automatically configure your server.',
        page: { setting_sections: { section_1: autoSettingPageObject } },
      },
    ];
    return pages.map((page) => {
      return createForm(page);
    });
  }, [setupManifest]);

  if (forms.length == 0 && step != 0) return <Spinner />;
  return (
    <>
      {step == 0 ? (
        <GameTypeSelectForm
          selectedGameType={gameType}
          setGameType={setGameType}
          urlValid={urlValid}
          setUrlValid={setUrlValid}
          setUrl={setUrl}
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
  const [urlIsReady, setUrlIsReady] = useState(false);
  const [urlValid, setUrlValid] = useState(true);
  const [url, setUrl] = useState<string>('');
  const {
    data: setup_manifest,
    isLoading,
    error,
  } = gameType === 'Generic'
    ? SetupGenericInstanceManifest(gameType, url, urlIsReady)
    : SetupInstanceManifest(gameType as HandlerGameType);

  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue | null>>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  useEffect(() => {
    setUrlValid(!(gameType === 'Generic' && error));
    if (gameType !== 'Generic') setUrlIsReady(false);
    if (!isLoading && !error) {
      setInitialValues(
        generateInitialValues(setup_manifest['setting_sections'])
      );
      setValidationSchema(generateValidationSchema(setup_manifest));
      // setup_manifest['setting_sections']['auto_settings'] =
      //   autoSettingPageObject;
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

  // if (setupManifest === null && activeStep !== 0) return <Spinner />;
  const currentValidationSchema = validationSchema[activeStep];
  const steps = [
    'Select Game',
    'Basic Settings',
    'Instance Settings',
    'Auto Settings',
  ];
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

    console.log(sectionValues);
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
    if (!setup_manifest || step == 0) return [{ settings: {} }, ''];
    const sectionKeys = Object.keys(setup_manifest['setting_sections']);
    const settingKeys = Object.keys(
      setup_manifest['setting_sections'][sectionKeys[step - 1]]['settings']
    );
    const sectionValidation: SectionManifestValue = { settings: {} };
    for (const key of settingKeys) {
      sectionValidation['settings'][key] = { value: values[key] };
    }
    return [sectionValidation, sectionKeys[step - 1]];
  }

  async function _handleSubmit(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    if (formReady) {
      _submitForm(values, actions);
    } else {
      if (activeStep == 0) setUrlIsReady(true);

      if (setup_manifest) {
        setActiveStep(activeStep + 1);
        actions.setValues(initialValues);
      }
      actions.setTouched({});
      actions.setSubmitting(false);
    }
  }

  function _handleBack() {
    if (activeStep - 1 == 0) setUrlIsReady(false);
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
      {({ isSubmitting }) => (
        <Form
          id={formId}
          className="flex max-h-[700px] min-h-[560px] w-[812px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg"
        >
          <div className="w-[180px] border-r border-gray-700 pt-8 ">
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
          <div className="flex w-[632px] flex-col p-8">
            {_renderStepContent(
              activeStep,
              gameType,
              setGameType,
              urlValid,
              setUrlValid,
              setUrl,
              setupManifest
            )}
            <div className="flex flex-row justify-between pt-6">
              {activeStep !== 0 ? (
                <Button onClick={_handleBack} label="Back" />
              ) : (
                <div></div>
              )}
              <Button
                type="submit"
                label={formReady ? 'Create Instance' : 'Next'}
                loading={isSubmitting || (urlIsReady && isLoading)}
              />
            </div>
          </div>
        </Form>
      )}
    </Formik>
  );
}
