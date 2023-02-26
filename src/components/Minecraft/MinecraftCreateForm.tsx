import { MinecraftFlavour } from 'bindings/MinecraftFlavour';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import Button from 'components/Atoms/Button';
import { Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState } from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper } from 'utils/util';
import {
  ConfigurableValue,
  formId,
  initialValues,
  MinecraftSetupConfigPrimitiveForm,
} from './Create/form';
import MinecraftAdvancedForm from './Create/MinecraftAdvancedForm';
import MinecraftBasicForm from './Create/MinecraftBasicForm';
import MinecraftNameForm from './Create/MinecraftNameForm';
import { flavourStringToMinecraftFlavour } from 'bindings/impl/FlavourStringToMinecraftFlavour';
import { generateValidationSchema, generateInitialValues } from './Create/form';
import { createForm } from './Create/FormCreation';
import MinecraftGameForm from './Create/MinecraftGameForm';
const sampleApiObject: any = {
  auto_start: false, //
  restart_on_crash: false, //
  start_on_connection: false, //
  setting_sections: {
    create_instance: {
      section_id: 'create_instance',
      name: 'Create an Instance',
      description:
        'Create a new Minecraft server instance to play with your friends.',
      settings: {
        name: {
          setting_id: 'name',
          name: 'Name',
          description: 'The name of the instance',
          value: {
            type: 'String',
            value: '',
          },
          value_type: {
            type: 'String',
            regex: null,
          },
          default_value: {
            type: 'String',
            value: '',
          },
          is_secret: false,
          is_required: true,
          is_mutable: true,
        },
      },
    },
    section_1: {
      section_id: 'section_1',
      name: 'Basic Settings',
      description: 'Basic settings for the server.',
      settings: {
        port: {
          setting_id: 'port',
          name: 'Port',
          description: 'The port to run the server on',
          value: {
            type: 'UnsignedInteger',
            value: 25565,
          },
          value_type: {
            type: 'UnsignedInteger',
            min: 0,
            max: 65535,
          },
          default_value: {
            type: 'UnsignedInteger',
            value: 25565,
          },
          is_secret: false,
          is_required: true,
          is_mutable: true,
        },
        version: {
          setting_id: 'version',
          name: 'Version',
          description: 'The version of minecraft to use',
          value: {
            type: 'Enum',
            value: '1.19.4-pre1',
          },
          value_type: {
            type: 'Enum',
            options: [
              '1.19.4-pre1',
              '23w07a',
              '23w06a',
              '23w05a',
              '23w04a',
              '23w03a',
              '1.19.3',
              '1.19.3-rc3',
              '1.19.3-rc2',
              '1.19.3-rc1',
              '1.19.3-pre3',
              '1.19.3-pre2',
              '1.19.3-pre1',
              '22w46a',
              '22w45a',
              '22w44a',
              '22w43a',
              '22w42a',
              '1.19.2',
              '1.19.2-rc2',
              '1.19.2-rc1',
              '1.19.1',
              '1.19.1-rc3',
              '1.19.1-rc2',
              '1.19.1-pre6',
              '1.19.1-pre5',
              '1.19.1-pre4',
              '1.19.1-pre3',
              '1.19.1-pre2',
              '1.19.1-rc1',
              '1.19.1-pre1',
              '22w24a',
              '1.19',
              '1.19-rc2',
              '1.19-rc1',
              '1.19-pre5',
              '1.19-pre4',
              '1.19-pre3',
              '1.19-pre2',
              '1.19-pre1',
              '22w19a',
              '22w18a',
              '22w17a',
              '22w16b',
              '22w16a',
              '22w15a',
              '22w14a',
              '22w13oneblockatatime',
              '22w13a',
              '22w12a',
              '22w11a',
              '1.18.2',
              '1.18.2-rc1',
              '1.18.2-pre3',
              '1.18.2-pre2',
              '1.18.2-pre1',
              '1.19_deep_dark_experimental_snapshot-1',
              '22w07a',
              '22w06a',
              '22w05a',
              '22w03a',
              '1.18.1',
              '1.18.1-rc3',
              '1.18.1-rc2',
              '1.18.1-rc1',
              '1.18.1-pre1',
              '1.18',
              '1.18-rc4',
              '1.18-rc3',
              '1.18-rc2',
              '1.18-rc1',
              '1.18-pre8',
              '1.18-pre7',
              '1.18-pre6',
              '1.18-pre5',
              '1.18-pre4',
              '1.18-pre3',
              '1.18-pre2',
              '1.18-pre1',
              '21w44a',
              '21w43a',
              '21w42a',
              '21w41a',
              '21w40a',
              '21w39a',
              '21w38a',
              '21w37a',
              '1.18_experimental-snapshot-7',
              '1.18_experimental-snapshot-6',
              '1.18_experimental-snapshot-5',
              '1.18_experimental-snapshot-4',
              '1.18_experimental-snapshot-3',
              '1.18_experimental-snapshot-2',
              '1.18_experimental-snapshot-1',
              '1.17.1',
              '1.17.1-rc2',
              '1.17.1-rc1',
              '1.17.1-pre3',
              '1.17.1-pre2',
              '1.17.1-pre1',
              '1.17',
              '1.17-rc2',
              '1.17-rc1',
              '1.17-pre5',
              '1.17-pre4',
              '1.17-pre3',
              '1.17-pre2',
              '1.17-pre1',
              '21w20a',
              '21w19a',
              '21w18a',
              '21w17a',
              '21w16a',
              '21w15a',
              '21w14a',
              '21w13a',
              '21w11a',
              '21w10a',
              '21w08b',
              '21w08a',
              '21w07a',
              '21w06a',
              '21w05b',
              '21w05a',
              '21w03a',
              '1.16.5',
              '1.16.5-rc1',
              '20w51a',
              '20w49a',
              '20w48a',
              '20w46a',
              '20w45a',
              '1.16.4',
              '1.16.4-rc1',
              '1.16.4-pre2',
              '1.16.4-pre1',
              '1.16.3',
              '1.16.3-rc1',
              '1.16_combat-3',
              '1.16.2',
              '1.16.2-rc2',
              '1.16.2-rc1',
              '1.16.2-pre3',
              '1.16.2-pre2',
              '1.16.2-pre1',
              '20w30a',
              '20w29a',
              '20w28a',
              '20w27a',
              '1.16.1',
              '1.16',
              '1.16-rc1',
              '1.16-pre8',
              '1.16-pre7',
              '1.16-pre6',
              '1.16-pre5',
              '1.16-pre4',
              '1.16-pre3',
              '1.16-pre2',
              '1.16-pre1',
              '20w22a',
              '20w21a',
              '20w20b',
              '20w20a',
              '20w19a',
              '20w18a',
              '20w17a',
              '20w16a',
              '20w15a',
              '20w14a',
              '20w14infinite',
              '20w13b',
              '20w13a',
              '20w12a',
              '20w11a',
              '20w10a',
              '20w09a',
              '20w08a',
              '20w07a',
              '20w06a',
              '1.15.2',
              '1.15.2-pre2',
              '1.15.2-pre1',
              '1.15.1',
              '1.15.1-pre1',
              '1.15',
              '1.15-pre7',
              '1.15-pre6',
              '1.15-pre5',
              '1.15-pre4',
              '1.15_combat-1',
              '1.15-pre3',
              '1.15-pre2',
              '1.15-pre1',
              '19w46b',
              '19w46a',
              '19w45b',
              '19w45a',
              '1.14_combat-3',
              '19w44a',
              '19w42a',
              '19w41a',
              '19w40a',
              '19w39a',
              '19w38b',
              '19w38a',
              '19w37a',
              '19w36a',
              '19w35a',
              '19w34a',
              '1.14_combat-0',
              '1.14.4',
              '1.14.4-pre7',
              '1.14.4-pre6',
              '1.14.4-pre5',
              '1.14.4-pre4',
              '1.14.4-pre3',
              '1.14.4-pre2',
              '1.14.4-pre1',
              '1.14.3',
              '1.14_combat-212796',
              '1.14.3-pre4',
              '1.14.3-pre3',
              '1.14.3-pre2',
              '1.14.3-pre1',
              '1.14.2',
              '1.14.2 Pre-Release 4',
              '1.14.2 Pre-Release 3',
              '1.14.2 Pre-Release 2',
              '1.14.2 Pre-Release 1',
              '1.14.1',
              '1.14.1 Pre-Release 2',
              '1.14.1 Pre-Release 1',
              '1.14',
              '1.14 Pre-Release 5',
              '1.14 Pre-Release 4',
              '1.14 Pre-Release 3',
              '1.14 Pre-Release 2',
              '1.14 Pre-Release 1',
              '19w14b',
              '19w14a',
              '3D Shareware v1.34',
              '19w13b',
              '19w13a',
              '19w12b',
              '19w12a',
              '19w11b',
              '19w11a',
              '19w09a',
              '19w08b',
              '19w08a',
              '19w07a',
              '19w06a',
              '19w05a',
              '19w04b',
              '19w04a',
              '19w03c',
              '19w03b',
              '19w03a',
              '19w02a',
              '18w50a',
              '18w49a',
              '18w48b',
              '18w48a',
              '18w47b',
              '18w47a',
              '18w46a',
              '18w45a',
              '18w44a',
              '18w43c',
              '18w43b',
            ],
          },
          default_value: null,
          is_secret: false,
          is_required: true,
          is_mutable: true,
        },
      },
    },
    section_2: {
      section_id: 'section_2',
      name: 'Advanced Settings',
      description: 'Advanced settings for your minecraft server.',
      settings: {
        cmd_args: {
          setting_id: 'cmd_args',
          name: 'Command Line Arguments',
          description: 'Command line arguments to pass to the server',
          value: {
            type: 'String',
            value: '',
          },
          value_type: {
            type: 'String',
            regex: null,
          },
          default_value: null,
          is_secret: false,
          is_required: false,
          is_mutable: true,
        },
        max_ram: {
          setting_id: 'max_ram',
          name: 'Maximum RAM',
          description: 'The maximum amount of RAM to allocate to the server',
          value: {
            type: 'UnsignedInteger',
            value: 2048,
          },
          value_type: {
            type: 'UnsignedInteger',
            min: null,
            max: null,
          },
          default_value: {
            type: 'UnsignedInteger',
            value: 2048,
          },
          is_secret: false,
          is_required: true,
          is_mutable: true,
        },
        min_ram: {
          setting_id: 'min_ram',
          name: 'Minimum RAM',
          description: 'The minimum amount of RAM to allocate to the server',
          value: {
            type: 'UnsignedInteger',
            value: 1024,
          },
          value_type: {
            type: 'UnsignedInteger',
            min: null,
            max: null,
          },
          default_value: {
            type: 'UnsignedInteger',
            value: 1024,
          },
          is_secret: false,
          is_required: true,
          is_mutable: true,
        },
      },
    },
  },
};

function _renderStepContent(step: number) {
  const forms = Object.keys(sampleApiObject['setting_sections']).map((key) =>
    createForm(sampleApiObject['setting_sections'][key])
  );

  const [gameType, setGameType] = useState({
    game: 'Minecraft',
    game_type: 'MinecraftVanilla',
  });
  console.log(gameType);
  switch (step) {
    case 0:
      return (
        <MinecraftGameForm gameType={gameType} setGameType={setGameType} />
      );
    case 1:
      return forms[0];
    case 2:
      return forms[1];
    case 3:
      return forms[2];
    default:
      return 'Unknown step';
  }
}

const steps = ['Game', 'Name', 'Basic', 'Advanced'];

export default function CreateMinecraftInstance({
  onComplete,
}: {
  onComplete: () => void;
}) {
  const [activeStep, setActiveStep] = useState(0);

  const initialValue = generateInitialValues(sampleApiObject);
  const validationscheme = generateValidationSchema(sampleApiObject);
  const currentValidationSchema = validationscheme[activeStep];
  const formReady = activeStep === steps.length - 1;
  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const formikRef =
    useRef<FormikProps<Record<string, ConfigurableValue>>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Instance Start');
  });

  const createInstance = async (value: Record<string, ConfigurableValue>) => {
    console.log(value);

    const flattenedData: Record<string, string | boolean | number | string[]> =
      {};

    for (const key in value) {
      flattenedData[key] = value[key].value;
    }

    console.log(flattenedData);
    await axiosWrapper<void>({
      method: 'post',
      url: '/instance/minecraft',
      headers: { 'Content-Type': 'application/json' },
      data: flattenedData,
    });
  };

  async function _submitForm(
    values: Record<string, ConfigurableValue>,
    actions: FormikHelpers<Record<string, ConfigurableValue>>
  ) {
    console.log(values);
    const parsedValues: Record<string, ConfigurableValue> = {
      ...values,
      // flavour: values.flavour as MinecraftFlavour,
      // cmd_args: values.cmd_args.split(' ').filter((item) => item !== ''),
      auto_start: {
        type: 'Boolean',
        value: values.auto_start.value === 'true',
      },
      restart_on_crash: {
        type: 'Boolean',
        value: values.restart_on_crash.value === 'true',
      },
      start_on_connection: {
        type: 'Boolean',
        value: values.start_on_connection.value === 'true',
      },
    };
    await createInstance(parsedValues);
    actions.setSubmitting(false);

    setActiveStep(activeStep + 1);
    gaEventTracker('Create Instance Complete');
    onComplete();
  }

  function _handleSubmit(
    values: Record<string, ConfigurableValue>,
    actions: FormikHelpers<Record<string, ConfigurableValue>>
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
