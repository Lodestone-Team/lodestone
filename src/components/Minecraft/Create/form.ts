import { axiosWrapper, catchAsyncToString } from 'utils/util';
import * as yup from 'yup';
import { PortStatus } from 'bindings/PortStatus';

export const formId = 'minecraftCreateNewInstanceForm';

export const generateValidationSchema = (instanceManifest:ConfigurableManifest) => {
  const validationSchema:any[] = []
  const setting_sections = instanceManifest["setting_sections"]
  validationSchema.push(yup.object().shape({})) //for select game type
  const generateYupObject = (setting: SettingManifest) => {
    const settingType = setting.value_type.type
      if(settingType === "String") {
        let validate = yup.string();
        if(setting.is_required) validate = validate.required(`${setting.name} is required`);
        return validate;
      } else if(settingType === "Integer" || settingType === "UnsignedInteger" || settingType === "Float") {
        let validate = yup.number();
        if(setting.is_required) validate = validate.required(`${setting.name} is required`);
        if(setting.value_type.min) validate = validate.min(setting.value_type.min, `${setting.name} must be greater than or equal to ${setting.value_type.min}`);
        if(setting.value_type.max) validate = validate.max(setting.value_type.max, `${setting.name} must be less than or equal to ${setting.value_type.max}`);
        return validate;
      } else if(settingType === "Boolean") {
        let validate = yup.boolean();
        if(setting.is_required) validate = validate.required(`${setting.name} is required`);
        return validate;
      } else if(settingType === "Enum") {
        let validate = yup.string().oneOf(setting.value_type.options, `${setting.name} must be one of the available options`);
        if(setting.is_required) validate = validate.required(`${setting.name} is required`);
        return validate;
      } else {
        throw Error("Invalid Setting Type");
      }
  }
  Object.keys(setting_sections).forEach((sectionId: string) => {
    const validationSchemaSection:Record<string, any> = {}
    const settings = setting_sections[sectionId]["settings"];
    Object.keys(settings).forEach((settingId: string) => {
      const setting = settings[settingId]
      validationSchemaSection[setting.setting_id] = yup.object().shape({value: generateYupObject(setting)})
    });
    validationSchema.push(yup.object().shape(validationSchemaSection))
  })
  return validationSchema
}


export const generateInitialValues = (settingSections:Record<string, SectionManifest>) => {
  const initialValues:Record<string, ConfigurableValue | null> = {};

  Object.keys(settingSections).forEach((sectionId: string) => {
    const setting = settingSections[sectionId]["settings"];
    Object.keys(setting).forEach((settingId: string) => {
      const settingValue = setting[settingId];
      initialValues[settingId] = settingValue.default_value ?? settingValue.value;
      if(initialValues[settingId] === null){
        if(settingValue.value_type.type === "Boolean") initialValues[settingId] = {type: "Boolean", value: false};
        else if(settingValue.value_type.type === "Integer") initialValues[settingId] = {type: "Integer", value: 0};
        else if(settingValue.value_type.type === "UnsignedInteger") initialValues[settingId] = {type: "UnsignedInteger", value: 0};
        else if(settingValue.value_type.type === "Float") initialValues[settingId] = {type: "Float", value: 0};
        else if(settingValue.value_type.type === "String") initialValues[settingId] = {type: "String", value: ""};
      }
    })
  })
  return initialValues
}

export interface ConfigurableManifest { auto_start: boolean, restart_on_crash: boolean, start_on_connection: boolean, setting_sections: Record<string, SectionManifest>, }
export interface ManifestValue { auto_start: boolean, restart_on_crash: boolean, start_on_connection: boolean, setting_sections: Record<string, SectionManifestValue>, }
export interface SectionManifest { section_id: string, name: string, description: string, settings: Record<string, SettingManifest>, }
export interface SectionManifestValue { settings: Record<string, SettingManifestValue>, }
export interface SettingManifest { setting_id: string, name: string, description: string, value: ConfigurableValue | null, value_type: ConfigurableValueType, default_value: ConfigurableValue | null, is_secret: boolean, is_required: boolean, is_mutable: boolean, }
export interface SettingManifestValue { value: ConfigurableValue | null, }
export type ConfigurableValueType = { type: "String", regex: string | null, } | { type: "Integer", min: number | null, max: number | null, } | { type: "UnsignedInteger", min: number | null, max: number | null, } | { type: "Float", min: number | null, max: number | null, } | { type: "Boolean" } | { type: "Enum", options: Array<string>, };
export type ConfigurableValue = { type: "String", value: string } | { type: "Integer", value: number } | { type: "UnsignedInteger", value: number } | { type: "Float", value: number } | { type: "Boolean", value: boolean } | { type: "Enum", value: string };