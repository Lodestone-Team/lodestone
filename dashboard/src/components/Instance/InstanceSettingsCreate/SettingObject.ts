import { faV } from '@fortawesome/free-solid-svg-icons';
import { ConfigurableManifest } from 'bindings/ConfigurableManifest';
import { ConfigurableValue } from 'bindings/ConfigurableValue';
import { ConfigurableValueType } from 'bindings/ConfigurableValueType';
import { SectionManifest } from 'bindings/SectionManifest';
import { SettingManifest } from 'bindings/SettingManifest';

export type SectionFieldObject = {
  section_id: string;
  name: string;
  description: string;
  settings: Record<string, SettingFieldObject>;
};

export type SettingFieldObject = {
  name: string;
  type: 'toggle' | 'number' | 'text' | 'dropdown' | 'password';
  options?: string[];
  description?: string;
  value: ConfigurableValue | null;
  min?: number | null;
  max?: number | null;
  is_mutable: boolean;
};

const getFieldType = (
  value_type: ConfigurableValueType,
  is_secret: boolean
) => {
  switch (value_type.type) {
    case 'Boolean':
      return 'toggle';
    case 'UnsignedInteger':
      return 'number';
    case 'Float':
      return 'text';
    case 'Integer':
      return 'number';
    case 'String':
      if (is_secret) return 'password';
      else return 'text';
    case 'Enum':
      return 'dropdown';
  }
};

export const generateSectionDataObject = (settingSection: SectionManifest) => {
  const settingsObject: Record<string, SettingFieldObject> = {};
  Object.keys(settingSection.settings).forEach((settingKey) => {
    const setting = settingSection.settings[settingKey];
    settingsObject[settingKey] = {
      name: setting.name,
      type: getFieldType(setting.value_type, setting.is_secret),
      description: setting.description,
      value: setting.value,
      is_mutable: setting.is_mutable,
    };
    if (setting.value_type.type === 'Enum')
      settingsObject[settingKey].options = setting.value_type.options;
    if (
      setting.value_type.type === 'UnsignedInteger' ||
      setting.value_type.type === 'Integer' ||
      setting.value_type.type === 'Float'
    ) {
      settingsObject[settingKey].min = setting.value_type.min;
      settingsObject[settingKey].max = setting.value_type.max;
    }
  });

  const sectionDataObject: SectionFieldObject = {
    section_id: settingSection.section_id,
    name: settingSection.name,
    description: settingSection.description,
    settings: settingsObject,
  };

  return sectionDataObject;
};

export const iterateSections = (manifest: ConfigurableManifest) => {
  const fieldSections: SectionFieldObject[] = [];
  Object.keys(manifest['setting_sections']).forEach((sectionKey) => {
    const section = manifest['setting_sections'][sectionKey];
    fieldSections.push(generateSectionDataObject(section));
  });
  return fieldSections;
};

export const adaptSettingManifest = (manifest: SettingManifest) => {
  const settingObject: SettingFieldObject = {
    name: manifest.name,
    type: getFieldType(manifest.value_type, manifest.is_secret),
    value: manifest.value,
    description :manifest.description,
    is_mutable: manifest.is_mutable
  }

  if (manifest.value_type.type == 'Enum') {
    settingObject.options = manifest.value_type.options
  }

  if (manifest.value_type.type === "UnsignedInteger" ||
      manifest.value_type.type === "Integer" ||
      manifest.value_type.type === "Float") {
      settingObject.min = manifest.value_type.min;
      settingObject.max = manifest.value_type.max;
  }
  return settingObject;
}
