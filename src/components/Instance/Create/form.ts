import { ConfigurableValue } from 'bindings/ConfigurableValue';
import { SectionManifest } from 'bindings/SectionManifest';
import { SettingManifest } from 'bindings/SettingManifest';
import { SetupManifest } from 'bindings/SetupManifest';
import * as yup from 'yup';

export const formId = 'minecraftCreateNewInstanceForm';

export const basicSettingsPageObject: SectionManifest = {
  section_id: 'basic_settings',
  name: 'Basic Settings',
  description: 'Basic settings for your server.',
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
        value: 'My Server',
      },
      is_secret: false,
      is_required: true,
      is_mutable: true,
    },
    description: {
      setting_id: 'description',
      name: 'Description',
      description: 'The description of the instance',
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
      is_required: false,
      is_mutable: true,
    },
  },
};

export const autoSettingPageObject: SectionManifest = {
  section_id: 'auto_settings',
  name: 'Auto Settings',
  description: 'Auto settings for your server.',
  settings: {
    auto_start: {
      setting_id: 'auto_start',
      name: 'Auto Start',
      description:
        'The instance will start automatically when the application starts',
      value: {
        type: 'Boolean',
        value: false,
      },
      value_type: {
        type: 'Boolean',
      },
      default_value: {
        type: 'Boolean',
        value: false,
      },
      is_secret: false,
      is_required: true,
      is_mutable: true,
    },
    restart_on_crash: {
      setting_id: 'restart_on_crash',
      name: 'Restart on Crash',
      description: 'The instance will restart automatically if it crashes',
      value: {
        type: 'Boolean',
        value: false,
      },
      value_type: {
        type: 'Boolean',
      },
      default_value: {
        type: 'Boolean',
        value: false,
      },
      is_secret: false,
      is_required: true,
      is_mutable: true,
    },
  },
};

export const generateValidationSchema = (instanceManifest: SetupManifest) => {
  const validationSchema: any[] = [];
  const setting_sections = instanceManifest['setting_sections'];

  validationSchema.push(yup.object().shape({})); //for select game type
  const generateYupObject = (setting: SettingManifest) => {
    const settingType = setting.value_type.type;
    if (settingType === 'String') {
      let validate = yup.string();
      if (setting.is_required)
        validate = validate.required(`${setting.name} is required`);
      return validate;
    } else if (
      settingType === 'Integer' ||
      settingType === 'UnsignedInteger' ||
      settingType === 'Float'
    ) {
      let validate = yup.number();
      if (setting.is_required)
        validate = validate.required(`${setting.name} is required`);
      if (setting.value_type.min)
        validate = validate.min(
          setting.value_type.min,
          `${setting.name} must be greater than or equal to ${setting.value_type.min}`
        );
      if (setting.value_type.max)
        validate = validate.max(
          setting.value_type.max,
          `${setting.name} must be less than or equal to ${setting.value_type.max}`
        );
      return validate;
    } else if (settingType === 'Boolean') {
      let validate = yup.boolean();
      if (setting.is_required)
        validate = validate.required(`${setting.name} is required`);
      return validate;
    } else if (settingType === 'Enum') {
      let validate = yup
        .string()
        .oneOf(
          setting.value_type.options,
          `${setting.name} must be one of the available options`
        );
      if (setting.is_required)
        validate = validate.required(`${setting.name} is required`);
      return validate;
    } else {
      throw Error('Invalid Setting Type');
    }
  };
  const instanceSettingsValidationSchemaSection: Record<string, any> = {};
  Object.keys(setting_sections).forEach((sectionId: string) => {
    const settings = setting_sections[sectionId]['settings'];
    Object.keys(settings).forEach((settingId: string) => {
      const setting = settings[settingId];
      instanceSettingsValidationSchemaSection[setting.setting_id] = yup
        .object()
        .shape({ value: generateYupObject(setting) });
    });
  });

  const basicSettingsValidationSchemaSection: Record<string, any> = {};
  Object.keys(basicSettingsPageObject['settings']).forEach(
    (settingId: string) => {
      const setting = basicSettingsPageObject['settings'][settingId];
      basicSettingsValidationSchemaSection[setting.setting_id] = yup
        .object()
        .shape({ value: generateYupObject(setting) });
    }
  );

  const autoSettingsValidationSchemaSection: Record<string, any> = {};
  Object.keys(autoSettingPageObject['settings']).forEach(
    (settingId: string) => {
      const setting = autoSettingPageObject['settings'][settingId];
      autoSettingsValidationSchemaSection[setting.setting_id] = yup
        .object()
        .shape({ value: generateYupObject(setting) });
    }
  );

  validationSchema.push(
    yup.object().shape(basicSettingsValidationSchemaSection)
  );
  validationSchema.push(
    yup.object().shape(instanceSettingsValidationSchemaSection)
  );
  validationSchema.push(
    yup.object().shape(autoSettingsValidationSchemaSection)
  );
  return validationSchema;
};

export const generateInitialValues = (
  settingSections: Record<string, SectionManifest>
) => {
  const initialValues: Record<string, ConfigurableValue | null> = {};
  const copySettingSections = { ...settingSections }; //don't modify original
  copySettingSections['basic_settings'] = basicSettingsPageObject;
  copySettingSections['auto_settings'] = autoSettingPageObject;
  const getInitialValue = (sectionId: string) => {
    const setting = copySettingSections[sectionId]['settings'];
    Object.keys(setting).forEach((settingId: string) => {
      const settingValue = setting[settingId];
      initialValues[settingId] =
        settingValue.default_value ?? settingValue.value;
      if (initialValues[settingId] === null) {
        if (settingValue.value_type.type === 'Boolean')
          initialValues[settingId] = { type: 'Boolean', value: false };
        else if (settingValue.value_type.type === 'Integer')
          initialValues[settingId] = { type: 'Integer', value: 0 };
        else if (settingValue.value_type.type === 'UnsignedInteger')
          initialValues[settingId] = { type: 'UnsignedInteger', value: 0 };
        else if (settingValue.value_type.type === 'Float')
          initialValues[settingId] = { type: 'Float', value: 0 };
        else if (settingValue.value_type.type === 'String')
          initialValues[settingId] = { type: 'String', value: '' };
      }
    });
  };
  Object.keys(copySettingSections).forEach((sectionId: string) => {
    getInitialValue(sectionId);
  });
  return initialValues;
};
