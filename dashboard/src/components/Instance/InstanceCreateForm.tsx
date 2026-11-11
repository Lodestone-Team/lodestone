import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState, useEffect, useMemo } from 'react';
import useAnalyticsEventTracker, { useEffectOnce } from 'utils/hooks';
import { axiosWrapper, catchAsyncToString, errorToString } from 'utils/util';
import { autoSettingsPage, basicSettingsPage, formId } from './Create/form';
import { generateValidationSchema, generateInitialValues } from './Create/form';
import { FormFromManifest } from './Create/FormFromManifest';
import GameTypeSelectForm from './Create/GameTypeSelectForm';
import { useSetupInstanceManifest } from 'data/InstanceGameTypes';
import { HandlerGameType } from 'bindings/HandlerGameType';
import WarningAlert from 'components/Atoms/WarningAlert';
import * as yup from 'yup';
import { GameInstanceContext } from 'data/GameInstanceContext';
import { ConfigurableValue } from 'bindings/ConfigurableValue';
import { SetupManifest } from 'bindings/SetupManifest';
import { SetupValue } from 'bindings/SetupValue';
import { SectionManifestValue } from 'bindings/SectionManifestValue';
import { toast } from 'react-toastify';
import { ModpackPreset } from 'data/InstanceContext';
import ModpackCreateForm from './Create/ModpackCreateForm';
import WizardSteps from './Create/WizardSteps';
import EulaCheckbox from './Create/EulaCheckbox';
import { useT } from 'i18n';

export type FormPage = {
  name: string;
  description: string;
  page: SetupManifest;
};

export default function InstanceCreateForm({
  onComplete,
  modpackPreset = null,
}: {
  onComplete: () => void;
  /** start on this Modrinth modpack instead of the game selection */
  modpackPreset?: ModpackPreset | null;
}) {
  const t = useT();
  const [activeStep, setActiveStep] = useState(0);
  // the "Modrinth modpack" flow replaces the manifest steps
  const [modpackMode, setModpackMode] = useState(modpackPreset !== null);
  // null until a game is picked
  const [gameType, setGameType] = useState<HandlerGameType | null>(null);
  // the user's own agreement to the Minecraft EULA: never ticked for them
  const [eulaAccepted, setEulaAccepted] = useState(false);
  const [setupManifest, setSetupManifest] = useState<SetupManifest | null>(
    null
  );
  // disabled (and pending) until a game is picked
  const {
    data: setup_manifest,
    isPending: isLoading,
    error,
  } = useSetupInstanceManifest(gameType as HandlerGameType, gameType !== null);

  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue | null>>>(null);

  const formPages = useMemo<FormPage[]>(() => {
    const basicPage: FormPage = {
      name: t('create.step.basicSettings'),
      description: t('create.basicSettings.description'),
      page: { setting_sections: { section_1: basicSettingsPage() } },
    };
    if (!setupManifest) return [basicPage];

    return [
      basicPage,
      {
        name: t('create.step.serverSettings'),
        description: t('create.serverSettings.description'),
        page: setupManifest,
      },
      {
        name: t('create.step.autoSettings'),
        description: t('create.autoSettings.description'),
        page: { setting_sections: { section_1: autoSettingsPage() } },
      },
    ];
  }, [setupManifest, t]);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  useEffect(() => {
    if (!isLoading && !error) {
      setInitialValues(
        generateInitialValues(setup_manifest['setting_sections'])
      );
      setValidationSchema(generateValidationSchema(setup_manifest));
      setSetupManifest(setup_manifest);
    }
  }, [gameType, isLoading, setup_manifest, error]);

  const [initialValues, setInitialValues] = useState<
    Record<string, ConfigurableValue | null>
  >({});
  const [validationSchema, setValidationSchema] = useState<any[]>([
    yup.object().shape({}),
  ]);

  const currentValidationSchema = validationSchema[activeStep];

  const sections = [
    t('create.step.selectGame'),
    t('create.step.basicSettings'),
    t('create.step.serverSettings'),
    t('create.step.autoSettings'),
  ];
  const formReady = activeStep === sections.length - 1;
  // every game this form creates is a Minecraft server, which needs the EULA
  // throws on failure
  const createInstance = async (value: SetupValue) => {
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
    const sectionValues = parseValues(values);

    const parsedValues: SetupValue = {
      name: values.name?.value as string,
      description: values.description?.value as string,
      auto_start: values.auto_start?.value as boolean,
      restart_on_crash: values.restart_on_crash?.value as boolean,
      setting_sections: sectionValues,
      accept_eula: eulaAccepted,
    };

    try {
      await createInstance(parsedValues);
    } catch (e) {
      // keep the dialog open so the user's input isn't lost
      toast.error(t('create.createFailed', { error: errorToString(e) }));
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
      if (!eulaAccepted) {
        actions.setSubmitting(false);
        return;
      }
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

  // Back to an earlier step; the values typed so far stay.
  function goToStep(step: number) {
    if (step >= activeStep) return;
    setActiveStep(step);
  }

  function handleBack() {
    goToStep(activeStep - 1);
  }

  if (modpackMode)
    return (
      <ModpackCreateForm
        preset={modpackPreset}
        onBack={() => setModpackMode(false)}
        onComplete={onComplete}
      />
    );

  return (
    <GameInstanceContext.Provider
      value={{
        gameType: gameType,
        setGameType: setGameType,
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
            <WizardSteps
              steps={sections}
              current={activeStep}
              onSelect={isSubmitting ? undefined : goToStep}
            />
            <div className="flex min-w-0 flex-1 flex-col p-8">
              {activeStep == 0 ? (
                <GameTypeSelectForm
                  onSelectModpack={() => setModpackMode(true)}
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
                        {t('create.checkFields')}
                        <br />
                        {status.error}
                      </p>
                    </WarningAlert>
                  )}
                </FormFromManifest>
              )}
              {formReady && (
                <div className="pt-6">
                  <EulaCheckbox
                    checked={eulaAccepted}
                    onChange={setEulaAccepted}
                    disabled={isSubmitting}
                  />
                </div>
              )}
              <div className="flex flex-row justify-between pt-6">
                {activeStep !== 0 ? (
                  <Button onClick={handleBack} label={t('common.back')} />
                ) : (
                  <div></div>
                )}
                <Button
                  type="submit"
                  label={
                    formReady ? t('create.createServer') : t('common.next')
                  }
                  loading={isSubmitting}
                  disabled={gameType === null || (formReady && !eulaAccepted)}
                />
              </div>
            </div>
          </Form>
        )}
      </Formik>
    </GameInstanceContext.Provider>
  );
}
