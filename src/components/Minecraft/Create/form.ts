import { MinecraftSetupConfigPrimitive } from 'bindings/MinecraftSetupConfigPrimitive';
import * as yup from 'yup';

export const formId = 'minecraftCreateNewInstanceForm';

export const initialValues: Optional<MinecraftSetupConfigPrimitive> = {
  name: '',
  version: '',
  flavour: '',
  port: 25565,
  cmd_args: [],
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

export const validationSchema = [
  yup.object().shape({
    name: yup.string().required('Name is required'),
  }),
  yup.object().shape({
    flavour: yup.string().required('Flavour is required'),
    version: yup.string().required('Version is required'),
    port: yup.number().required('Port is required'),
  }),
  yup.object().shape({
    min_ram: yup.number().default(128).required('Min RAM is required'),
    max_ram: yup.number().default(1024).required('Max RAM is required'),
    cmd_args: yup.array().of(yup.string()),
    auto_start: yup.boolean(),
    restart_on_crash: yup.boolean(),
  }),
];
