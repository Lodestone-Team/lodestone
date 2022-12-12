// a wrapper around TextField that fetches a single setting from the server
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useGameSetting } from 'data/GameSetting';
import { axiosPutSingleValue, errorToMessage } from 'utils/util';
import Dropdown from './Atoms/Config/SelectBox';
import InputBox from './Atoms/Config/InputBox';
import { useState } from 'react';
import { useIsomorphicLayoutEffect } from 'usehooks-ts';
import { useUserAuthorized } from 'data/UserInfo';
import ToggleBox from './Atoms/Config/ToggleBox';

export default function SettingField({
  instance,
  setting,
  label,
  type = 'text',
  options,
  min,
  max,
  description,
  descriptionFunc,
}: {
  instance: InstanceInfo;
  setting: string;
  label?: string;
  type?: 'text' | 'number' | 'dropdown' | 'toggle';
  min?: number;
  max?: number;
  options?: string[];
  description?: React.ReactNode;
  descriptionFunc?: (arg: any) => React.ReactNode;
}) {
  const uuid = instance.uuid;
  const can_access_instance_setting = useUserAuthorized(
    'can_access_instance_setting',
    instance.uuid
  );
  const {
    data: initialSetting,
    isLoading: isSettingLoading,
    error,
  } = useGameSetting(uuid, setting, can_access_instance_setting);
  label = label ?? setting;
  const [value, setValue] = useState(initialSetting ?? '');


  useIsomorphicLayoutEffect(() => {
    setValue(initialSetting ?? '');
  }, [initialSetting]);

  const errorString = errorToMessage(error);
  const isLoading = can_access_instance_setting ? isSettingLoading : false;

  switch (type) {
    case 'text':
      return (
        <InputBox
          label={label}
          value={value}
          type="text"
          isLoading={isLoading}
          error={errorString}
          canRead={can_access_instance_setting}
          onSubmit={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value
            );
            setValue(value);
          }}
          description={description}
          descriptionFunc={descriptionFunc}
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
          isLoading={isLoading}
          error={errorString}
          canRead={can_access_instance_setting}
          onSubmit={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value
            );
            // print type of value
            console.log(typeof value);
            setValue(value);
          }}
          description={description}
          descriptionFunc={descriptionFunc}
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
          isLoading={isLoading}
          error={errorString}
          canRead={can_access_instance_setting}
          onChange={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value
            );
          }}
          description={description}
          descriptionFunc={descriptionFunc}
        />
      );
    case 'toggle':
      return (
        <ToggleBox
          label={label}
          value={value === 'true'}
          isLoading={isLoading}
          error={errorString}
          canRead={can_access_instance_setting}
          onChange={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value ? 'true' : 'false'
            );
          }}
          description={description}
          descriptionFunc={descriptionFunc}
        />
      );
  }
}
