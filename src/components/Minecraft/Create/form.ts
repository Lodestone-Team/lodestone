import { axiosWrapper, catchAsyncToString } from 'utils/util';
import * as yup from 'yup';
import { PortStatus } from 'bindings/PortStatus';

export const formId = 'minecraftCreateNewInstanceForm';

export type MinecraftSetupConfigPrimitiveForm = {
  name: string;
  version: string; //
  flavour: string;
  port: number; //
  cmd_args: string; //
  description: string;
  fabric_loader_version: string | null;
  fabric_installer_version: string | null;
  min_ram: number; //
  max_ram: number; //
  auto_start: string; //
  restart_on_crash: string; //
  timeout_last_left: number | null;
  timeout_no_activity: number | null;
  start_on_connection: string; //
  backup_period: number | null;
};

export const initialValues: MinecraftSetupConfigPrimitiveForm = {
  name: '',
  version: '',
  flavour: '',
  port: 25565,
  cmd_args: '',
  description: 'Pizza is good',
  fabric_loader_version: null,
  fabric_installer_version: null,
  min_ram: 1024,
  max_ram: 4096,
  auto_start: 'false',
  restart_on_crash: 'false',
  timeout_last_left: null,
  timeout_no_activity: null,
  start_on_connection: 'false',
  backup_period: null,
};

// const checkPortValid = async (port: number) => {
//   const result = await axiosWrapper<PortStatus>({
//     method: 'get',
//     url: `/check/port/${port}`,
//   });
//   if (result.is_allocated) throw new Error('Port is used');
// };

// export const validationSchema = [
//   yup.object().shape({
//     name: yup.string().required('Name is required'),
//   }),
//   yup.object().shape({
//     flavour: yup.string().required('Flavour is required'),
//     version: yup.string().required('Version is required'),
//     port: yup
//       .number()
//       .required('Port is required')
//       .test('port range', 'must be between 1 and 65535', async (value) => {
//         if (value === undefined) return false;
//         return value >= 1 && value <= 65535;
//       })
//       .test('port free', 'Port not available', async (value) => {
//         if (value === undefined) return false;
//         return !(await catchAsyncToString(checkPortValid(value)));
//       }),
//   }),
//   yup.object().shape({
//     min_ram: yup.number().required('Min RAM is required'),
//     max_ram: yup.number().required('Max RAM is required'),
//     cmd_args: yup.string().nullable(),
//     auto_start: yup.boolean(),
//     restart_on_crash: yup.boolean(),
//   }),
// ];

// export const validationSchema = [
//   yup.object().shape({
//     name: yup.string().required('Name is required'),
//   }),
//   yup.object().shape({
//     version: yup.string().required('Version is required'),
//     port: yup
//       .number()
//       .required('Port is required')
//       .test('port range', 'must be between 1 and 65535', async (value) => {
//         if (value === undefined) return false;
//         return value >= 1 && value <= 65535;
//       })
//       .test('port free', 'Port not available', async (value) => {
//         if (value === undefined) return false;
//         return !(await catchAsyncToString(checkPortValid(value)));
//       }),
//   }),
//   yup.object().shape({
//     min_ram: yup.number().required('Min RAM is required'),
//     max_ram: yup.number().required('Max RAM is required'),
//     cmd_args: yup.string().nullable(),
//     auto_start: yup.boolean(),
//     restart_on_crash: yup.boolean(),
//   }),
// ];

export const generateValidationSchema = (sampleApiObject:Record<string, any>) => {
  const validationSchema:any[] = []
  const setting_sections = sampleApiObject["setting_sections"]
  validationSchema.push(yup.object().shape({}))
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


export const generateInitialValues = (sampleApiObject:Record<string, any>) => {
  const initialValues:Record<string, ConfigurableValue> = {};
  Object.keys(sampleApiObject).forEach((key: string) => {
    if(key !== "setting_sections") initialValues[key] = sampleApiObject[key];
    else{
      const settingSections = sampleApiObject[key];
      Object.keys(settingSections).forEach((sectionId: string) => {
        const setting = settingSections[sectionId]["settings"];
        Object.keys(setting).forEach((settingId: string) => {
          const settingValue = setting[settingId];
          initialValues[settingId] = settingValue.default_value ?? settingValue.value;
        })
      })
    }
  });
  return initialValues
}


export interface SectionManifest { section_id: string, name: string, description: string, settings: Record<string, SettingManifest>, }
export interface SectionManifestValue { settings: Record<string, SettingManifestValue>, }
export interface SettingManifest { setting_id: string, name: string, description: string, value: ConfigurableValue | null, value_type: ConfigurableValueType, default_value: ConfigurableValue | null, is_secret: boolean, is_required: boolean, is_mutable: boolean, }
export interface SettingManifestValue { value: ConfigurableValue | null, }
export type ConfigurableValueType = { type: "String", regex: string | null, } | { type: "Integer", min: number | null, max: number | null, } | { type: "UnsignedInteger", min: number | null, max: number | null, } | { type: "Float", min: number | null, max: number | null, } | { type: "Boolean" } | { type: "Enum", options: Array<string>, };
export type ConfigurableValue = { type: "String", value: string } | { type: "Integer", value: number } | { type: "UnsignedInteger", value: number } | { type: "Float", value: number } | { type: "Boolean", value: boolean } | { type: "Enum", value: string };