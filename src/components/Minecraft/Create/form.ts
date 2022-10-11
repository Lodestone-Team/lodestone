import { axiosWrapper, catchAsyncToString } from 'utils/util';
import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import * as yup from 'yup';

export const formId = 'minecraftCreateNewInstanceForm';

export const initialValues: MinecraftSetupConfigPrimitive = {
  name: '',
  version: '',
  flavour: 'vanilla',
  port: 25565,
  cmd_args: [] as string[],
  description: 'Pizza is good',
  fabric_loader_version: null,
  fabric_installer_version: null,
  min_ram: 128,
  max_ram: 1024,
  auto_start: false,
  restart_on_crash: false,
  timeout_last_left: null,
  timeout_no_activity: null,
  start_on_connection: null,
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
    name: yup.string().required('Name is required'),
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
    cmd_args: yup.array().of(yup.string()).nullable(),
    auto_start: yup.boolean(),
    restart_on_crash: yup.boolean(),
  }),
];
