// a wrapper around TextField that fetches a single setting from the server
import { InstanceInfo } from 'bindings/InstanceInfo';
import { axiosPutSingleValue, errorToString } from 'utils/util';
import Dropdown from './Atoms/Config/SelectBox';
import InputBox from './Atoms/Config/InputBox';
import { useState } from 'react';
import { useIsomorphicLayoutEffect } from 'usehooks-ts';
import { useUserAuthorized, useUserInfo } from 'data/UserInfo';
import { useGlobalSettings } from 'data/GlobalSettings';
import ToggleBox from './Atoms/Config/ToggleBox';
import { ConfigurableValue } from 'bindings/ConfigurableValue';
import { AxiosError } from 'axios';
import { toast } from 'react-toastify';
import { SettingFieldObject } from './Instance/InstanceSettingsCreate/SettingObject';

// Settings that choose the program an instance runs and its arguments. The
// core answers 403 to anyone but the owner, or, with safe mode off, a user
// allowed to write global files (handlers/instance_config.rs).
const COMMAND_LINE_SETTINGS = ['java_cmd', 'cmd_args'];

const useCanChangeCommandLine = () => {
  const { data: user } = useUserInfo();
  const { data: globalSettings } = useGlobalSettings();
  if (!user) return false;
  if (user.is_owner) return true;
  return (
    user.permissions.can_write_global_file &&
    globalSettings?.safe_mode === false
  );
};

export default function SettingField({
  instance,
  setting,
  sectionId,
  settingId,
  error,
  descriptionFunc,
  onSubmit
}: {
  instance: InstanceInfo;
  setting: SettingFieldObject;
  sectionId: string;
  settingId: string;
  error?: AxiosError<unknown, any> | null;
  descriptionFunc?: (arg: string | boolean) => React.ReactNode,
  onSubmit?: (updated: ConfigurableValue | null) => void
}) {
  const { name: label, type, options, value: initialValue, min, max } = setting;
  const canChangeCommandLine = useCanChangeCommandLine();
  const ownerOnly =
    COMMAND_LINE_SETTINGS.includes(settingId) && !canChangeCommandLine;
  const is_mutable = setting.is_mutable && !ownerOnly;
  const description = ownerOnly ? (
    <>
      {setting.description}
      <span className="block italic">
        Only the owner can change this setting.
      </span>
    </>
  ) : (
    setting.description
  );
  const uuid = instance.uuid;
  const can_access_instance_setting = useUserAuthorized(
    'can_access_instance_setting',
    instance.uuid
  );
  const initialString = initialValue ? initialValue.value.toString() : '';
  const [value, setValue] = useState(initialString);

  // keep the field in sync when the setting changes from outside
  useIsomorphicLayoutEffect(() => {
    setValue(initialString);
  }, [initialString]);

  const errorString = errorToString(error);
  const URL = `/instance/${uuid}/settings/${sectionId}/${settingId}`;
  const submitSettingField = async (value: string) => {
    const getConfigurableValue: (value: string) => ConfigurableValue | null = (
      value: string
    ) => {
      switch (type) {
        case 'text':
          if (initialValue?.type === 'Float')
            return { type: 'Float', value: parseFloat(value) };
          else return { type: 'String', value: value };
        case 'password':
          return { type: 'String', value: value };
        case 'number':
          if (initialValue?.type === 'Integer')
            return { type: 'Integer', value: parseInt(value) };
          else return { type: 'UnsignedInteger', value: parseInt(value) };
        case 'toggle':
          return { type: 'Boolean', value: value === 'true' };
        case 'dropdown':
          return { type: 'Enum', value: value };
        default:
          toast.error('Submission Error: Unknown value type.');
          return null;
      }
    };
    await (onSubmit? onSubmit(getConfigurableValue(value)) : axiosPutSingleValue<void>(`${URL}`, getConfigurableValue(value)));
  };
  // const isLoading = can_access_instance_setting ? isSettingLoading : false;

  switch (type) {
    case 'text':
      return (
        <InputBox
          label={label}
          value={value}
          type="text"
          isFloat={initialValue?.type === 'Float'}
          // isLoading={isLoading}
          error={errorString}
          canRead={can_access_instance_setting}
          onSubmit={async (value) => {
            await submitSettingField(value);
            setValue(value);
          }}
          description={description}
          descriptionFunc={descriptionFunc}
          disabled={!is_mutable}
        />
      );
    case 'password':
      return (
        <InputBox
          label={label}
          value={value}
          type="password"
          error={errorString}
          canRead={can_access_instance_setting}
          onSubmit={async (value) => {
            await submitSettingField(value);
            setValue(value);
          }}
          description={description}
          descriptionFunc={descriptionFunc}
          disabled={!is_mutable}
        />
      );
    case 'number':
      return (
        <InputBox
          label={label}
          value={value}
          type="number"
          min={min}
          max={max}
          error={errorString}
          canRead={can_access_instance_setting}
          onSubmit={async (value) => {
            await submitSettingField(value);
            setValue(value);
          }}
          description={description}
          descriptionFunc={descriptionFunc}
          disabled={!is_mutable}
        />
      );
    case 'dropdown':
      if (!options) {
        throw new Error('Dropdown type requires options');
      }
      return (
        <Dropdown
          label={label}
          value={value}
          options={options}
          error={errorString}
          canRead={can_access_instance_setting}
          onChange={submitSettingField}
          description={description}
          descriptionFunc={descriptionFunc}
          disabled={!is_mutable}
        />
      );
    case 'toggle':
      return (
        <ToggleBox
          label={label}
          value={value === 'true'}
          error={errorString}
          canRead={can_access_instance_setting}
          onChange={async (value) => {
            await (onSubmit? onSubmit({
              type: 'Boolean',
              value: value,
            }) : axiosPutSingleValue<void>(`${URL}`, {
              type: 'Boolean',
              value: value,
            }));
          }}
          description={description}
          descriptionFunc={descriptionFunc}
          disabled={!is_mutable}
        />
      );
  }
}
