import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState, useEffect, useMemo } from 'react';
import useAnalyticsEventTracker, { useEffectOnce } from 'utils/hooks';
import { axiosWrapper, catchAsyncToString, errorToString } from 'utils/util';
import {
  autoSettingPageObject,
  basicSettingsPageObject,
  formId,
} from './Create/form';
import { generateValidationSchema, generateInitialValues } from './Create/form';
import { FormFromManifest } from './Create/FormFromManifest';
import GameTypeSelectForm from './Create/GameTypeSelectForm';
import {
  useGenericSetupInstanceManifest,
  useSetupInstanceManifest,
} from 'data/InstanceGameTypes';
import { HandlerGameType } from 'bindings/HandlerGameType';
import Spinner from 'components/DashboardLayout/Spinner';
import WarningAlert from 'components/Atoms/WarningAlert';
import clsx from 'clsx';
import * as yup from 'yup';
import { GameInstanceContext } from 'data/GameInstanceContext';
import { ConfigurableValue } from 'bindings/ConfigurableValue';
import { SetupManifest } from 'bindings/SetupManifest';
import { SetupValue } from 'bindings/SetupValue';
import { SectionManifestValue } from 'bindings/SectionManifestValue';
import { toast } from 'react-toastify';

export type GenericHandlerGameType = 'Generic' | HandlerGameType;
export type FormPage = {
  name: string;
  description: string;
  page: SetupManifest;
};

export default function InstanceCreateForm({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [activeStep, setActiveStep] = useState(0);
  const [gameType, setGameType] = useState<GenericHandlerGameType>('Generic');
  const [genericFetchReady, setGenericFetchReady] = useState(false); //if the button has been pressed to fetch the manifest -> enables the query
  const [urlValid, setUrlValid] = useState(false); //if the query returned a valid manifest
  const [url, setUrl] = useState<string>(''); //the url the user enters
  const [setupManifest, setSetupManifest] = useState<SetupManifest | null>(
    null
  );
  // Call both hooks every render (rules of hooks); only the one for the
  // selected game type is enabled.
  const genericManifestQuery = useGenericSetupInstanceManifest(
    gameType,
    url,
    gameType === 'Generic' && genericFetchReady
  );
  const gameManifestQuery = useSetupInstanceManifest(
    gameType as HandlerGameType,
    gameType !== 'Generic'
  );
  const {
    data: setup_manifest,
    isPending: isLoading,
    error,
  } = gameType === 'Generic' ? genericManifestQuery : gameManifestQuery;

  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue | null>>>(null);

  const formPages = useMemo<FormPage[]>(() => {
    if (!setupManifest)
      return [
        {
          name: 'Basic Settings',
          description: 'Basic settings for your server.',
          page: { setting_sections: { section_1: basicSettingsPageObject } },
        },
      ];

    return [
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
  }, [setupManifest]);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  useEffect(() => {
    if (gameType !== 'Generic') {
      setGenericFetchReady(false);
      setUrlValid(false);
    }
    if (!isLoading && !error) {
      if (gameType === 'Generic' && genericFetchReady) setUrlValid(true); //value fetched with no errors (this is to cover the initial case when nothing has been fetched yet)
      setInitialValues(
        generateInitialValues(setup_manifest['setting_sections'])
      );
      setValidationSchema(generateValidationSchema(setup_manifest));
      setSetupManifest(setup_manifest);
    }
  }, [gameType, isLoading, setup_manifest, error, genericFetchReady]);



  const [initialValues, setInitialValues] = useState<
    Record<string, ConfigurableValue | null>
  >({});
  const [validationSchema, setValidationSchema] = useState<any[]>([
    yup.object().shape({}),
  ]);

  // if (setupManifest === null && activeStep !== 0) return <Spinner />;
  const currentValidationSchema = validationSchema[activeStep];

  const sections = [
    'Select Game',
    'Basic Settings',
    'Instance Settings',
    'Auto Settings',
  ];
  const formReady = activeStep === sections.length - 1;
  // throws on failure
  const createInstance = async (value: SetupValue) => {
    if (gameType === 'Generic') {
      // Left over from the removed atom flow. The Docker card has no
      // container name field, so this step is never reached; the Docker
      // flow's only request is the watch-list add in
      // useGenericSetupInstanceManifest.
      await axiosWrapper<void>({
        method: 'post',
        url: `/instance/create_generic`,
        headers: { 'Content-Type': 'application/json' },
        data: JSON.stringify({ url: url, setup_value: value }),
      });
    } else {
      await axiosWrapper<void>({
        method: 'post',
        url: `/instance/create/${gameType}`,
        headers: { 'Content-Type': 'application/json' },
        data: JSON.stringify(value),
      });
    }
  };

  async function submitForm(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    const sectionValues = parseValues(values);

    const parsedValues: SetupValue = {
      name: values.name?.value as string,
      description: values.description?.value as string,
      auto_start: values.auto_start?.value as boolean,
      restart_on_crash: values.restart_on_crash?.value as boolean,
      setting_sections: sectionValues,
    };

    try {
      await createInstance(parsedValues);
    } catch (e) {
      // keep the dialog open so the user's input isn't lost
      toast.error('Error creating instance: ' + errorToString(e));
      actions.setSubmitting(false);
      return;
    }
    actions.setSubmitting(false);

    gaEventTracker('Create Instance Complete');
    onComplete();
  }

  function parseValues(
    values: Record<string, ConfigurableValue | null>
  ): Record<string, SectionManifestValue> {
    if (!setup_manifest) return {};
    const sectionKeys = Object.keys(setup_manifest['setting_sections']);

    const sectionValues: Record<string, SectionManifestValue> = {};

    for (const sectionKey of sectionKeys) {
      const sectionValue: SectionManifestValue = { settings: {} };
      const settingKeys = Object.keys(
        setup_manifest['setting_sections'][sectionKey]['settings']
      );
      for (const key of settingKeys) {
        sectionValue['settings'][key] = { value: values[key] };
      }
      sectionValues[sectionKey] = sectionValue;
    }

    return sectionValues;
  }

  async function handleSubmit(
    values: Record<string, ConfigurableValue | null>,
    actions: FormikHelpers<Record<string, ConfigurableValue | null>>
  ) {
    if (formReady) {
      await submitForm(values, actions);
    } else {
      if (setup_manifest) {
        if (activeStep === 0) actions.setValues(initialValues);
        setActiveStep(activeStep + 1);
      }
      actions.setTouched({});
      actions.setSubmitting(false);
    }
  }

  function handleBack() {
    if(activeStep === 1) {
      setGenericFetchReady(false);
      setUrlValid(false);
      setUrl('');
    }
    setActiveStep(activeStep - 1);
  }

  return (
    <GameInstanceContext.Provider
      value={{
        gameType: gameType,
        setGameType: setGameType,
        url: url,
        setUrl: setUrl,
        urlValid: urlValid,
        setUrlValid: setUrlValid,
        genericFetchReady: genericFetchReady,
        setGenericFetchReady: setGenericFetchReady,
      }}
    >
      <Formik
        initialValues={initialValues}
        validationSchema={currentValidationSchema}
        onSubmit={handleSubmit}
        innerRef={formikRef}
        validateOnBlur={false}
        validateOnChange={false}
      >
        {({ isSubmitting, status }) => (
          <Form
            id={formId}
            className="flex max-h-[700px] min-h-[560px] w-[812px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg"
          >
            <div className="w-[180px] border-r border-gray-700 pt-8 ">
              {sections.map((section, i) => (
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
              {activeStep == 0 ? (
                <GameTypeSelectForm
                  manifestLoading={isLoading}
                  manifestError={!!error}
                />
              ) : (
                <FormFromManifest
                  name={formPages[activeStep - 1].name}
                  description={formPages[activeStep - 1].description}
                  manifest={formPages[activeStep - 1].page}
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
              <div className="flex flex-row justify-between pt-6">
                {activeStep !== 0 ? (
                  <Button onClick={handleBack} label="Back" />
                ) : (
                  <div></div>
                )}
                <Button
                  type="submit"
                  label={formReady ? 'Create Instance' : 'Next'}
                  loading={isSubmitting}
                  disabled={gameType === 'Generic' && !urlValid}
                />
              </div>
            </div>
          </Form>
        )}
      </Formik>
    </GameInstanceContext.Provider>
  );
}
