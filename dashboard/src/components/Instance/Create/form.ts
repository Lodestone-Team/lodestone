import { ConfigurableValue } from 'bindings/ConfigurableValue';
import { SectionManifest } from 'bindings/SectionManifest';
import { SettingManifest } from 'bindings/SettingManifest';
import { SetupManifest } from 'bindings/SetupManifest';
import { t } from 'i18n';
import * as yup from 'yup';

export const formId = 'minecraftCreateNewInstanceForm';

// Functions, not constants: the texts are in the current language when called
export const basicSettingsPage = (): SectionManifest => ({
  section_id: 'basic_settings',
  name: t('create.step.basicSettings'),
  description: t('create.basicSettings.description'),
  settings: {
    name: {
      setting_id: 'name',
      name: t('create.field.name'),
      description: t('create.field.nameDescription'),
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
        value: t('create.field.nameDefault'),
      },
      is_secret: false,
      is_required: true,
      is_mutable: true,
    },
    description: {
      setting_id: 'description',
      name: t('create.field.description'),
      description: t('create.field.descriptionDescription'),
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
});

export const autoSettingsPage = (): SectionManifest => ({
  section_id: 'auto_settings',
  name: t('create.step.autoSettings'),
  description: t('create.autoSettings.description'),
  settings: {
    auto_start: {
      setting_id: 'auto_start',
      name: t('create.field.autoStart'),
      description: t('create.field.autoStartDescription'),
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
      name: t('create.field.restartOnCrash'),
      description: t('create.field.restartOnCrashDescription'),
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
});

export const generateValidationSchema = (instanceManifest: SetupManifest) => {
  const validationSchema: any[] = [];
  const setting_sections = instanceManifest['setting_sections'];

  validationSchema.push(yup.object().shape({})); //for select game type
  // the messages are functions, so yup writes them in the language of the
  // moment it validates
  const generateYupObject = (setting: SettingManifest) => {
    const settingType = setting.value_type.type;
    const name = setting.name ?? '';
    const required = () => t('create.validation.required', { name });
    const integer = () => t('create.validation.integer', { name });
    if (settingType === 'String') {
      let validate = yup.string();
      if (setting.is_required) validate = validate.required(required);
      return validate;
    } else if (
      settingType === 'Integer' ||
      settingType === 'UnsignedInteger' ||
      settingType === 'Float'
    ) {
      let validate = yup
        .number()
        .typeError(() => t('create.validation.number', { name }));
      if (settingType === 'Integer') validate = validate.integer(integer);
      if (settingType === 'UnsignedInteger')
        validate = validate
          .integer(integer)
          .min(0, () => t('create.validation.positive', { name }));
      if (setting.is_required) validate = validate.required(required);
      const { min, max } = setting.value_type;
      if (min)
        validate = validate.min(min, () =>
          t('create.validation.min', { name, min })
        );
      if (max)
        validate = validate.max(max, () =>
          t('create.validation.max', { name, max })
        );
      return validate;
    } else if (settingType === 'Boolean') {
      let validate = yup.boolean();
      if (setting.is_required) validate = validate.required(required);
      return validate;
    } else if (settingType === 'Enum') {
      let validate = yup
        .string()
        .oneOf(setting.value_type.options, () =>
          t('create.validation.oneOf', { name })
        );
      if (setting.is_required) validate = validate.required(required);
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

  const basicSettings = basicSettingsPage();
  const basicSettingsValidationSchemaSection: Record<string, any> = {};
  Object.keys(basicSettings['settings']).forEach((settingId: string) => {
    const setting = basicSettings['settings'][settingId];
    basicSettingsValidationSchemaSection[setting.setting_id] = yup
      .object()
      .shape({ value: generateYupObject(setting) });
  });

  const autoSettings = autoSettingsPage();
  const autoSettingsValidationSchemaSection: Record<string, any> = {};
  Object.keys(autoSettings['settings']).forEach((settingId: string) => {
    const setting = autoSettings['settings'][settingId];
    autoSettingsValidationSchemaSection[setting.setting_id] = yup
      .object()
      .shape({ value: generateYupObject(setting) });
  });

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
  copySettingSections['basic_settings'] = basicSettingsPage();
  copySettingSections['auto_settings'] = autoSettingsPage();
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
