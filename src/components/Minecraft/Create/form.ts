import { axiosWrapper, catchAsyncToString } from 'utils/util';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import * as yup from 'yup';
import { MinecraftFlavour } from 'bindings/MinecraftFlavour';

export const formId = 'minecraftCreateNewInstanceForm';

export type MinecraftSetupConfigPrimitiveForm = {
  name: string;
  version: string;
  flavour: string;
  port: number;
  cmd_args: string;
  description: string;
  fabric_loader_version: string | null;
  fabric_installer_version: string | null;
  min_ram: number;
  max_ram: number;
  auto_start: string;
  restart_on_crash: string;
  timeout_last_left: number | null;
  timeout_no_activity: number | null;
  start_on_connection: string;
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

const checkPortValid = async (port: number) => {
  const result = await axiosWrapper<boolean>({
    method: 'get',
    url: `/check/port/${port}`,
  });
  if (result) throw new Error('Port is used');
};

export const validationSchema = [
  yup.object().shape({
    name: yup.string().required('Name is required'),
  }),
  yup.object().shape({
    flavour: yup.string().required('Flavour is required'),
    version: yup.string().required('Version is required'),
    port: yup
      .number()
      .required('Port is required')
      .test('port range', 'must be between 1 and 65535', async (value) => {
        if (value === undefined) return false;
        return value >= 1 && value <= 65535;
      })
      .test('port free', 'Port in use', async (value) => {
        if (value === undefined) return false;
        return !(await catchAsyncToString(checkPortValid(value)));
      }),
  }),
  yup.object().shape({
    min_ram: yup.number().required('Min RAM is required'),
    max_ram: yup.number().required('Max RAM is required'),
    cmd_args: yup.string().nullable(),
    auto_start: yup.boolean(),
    restart_on_crash: yup.boolean(),
  }),
];
