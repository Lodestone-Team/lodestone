// a wrapper around TextField that fetches a single setting from the server
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useGameSetting } from 'data/GameSetting';
import { axiosPutSingleValue, errorToMessage } from 'utils/util';
import Dropdown from './Atoms/Config/SelectBox';
import Textfield from './Atoms/Config/InputBox';
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
}: {
  instance: InstanceInfo;
  setting: string;
  label?: string;
  type?: 'text' | 'number' | 'dropdown' | 'toggle';
  min?: number;
  max?: number;
  options?: string[];
}) {
  const uuid = instance.uuid;
  const {
    data: initialSetting,
    isLoading,
    error,
  } = useGameSetting(uuid, setting);
  label = label ?? setting;
  const [value, setValue] = useState(initialSetting ?? '');
  const can_access_instance_setting = useUserAuthorized(
    'can_access_instance_setting',
    instance.uuid
  );

  useIsomorphicLayoutEffect(() => {
    setValue(initialSetting ?? '');
  }, [initialSetting]);

  console.log("value in settingfield" + value);

  const errorString = errorToMessage(error);

  switch (type) {
    case 'text':
      return (
        <Textfield
          label={label}
          value={value}
          type="text"
          disabled={isLoading}
          error={errorString}
          onSubmit={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value
            );
            setValue(value);
          }}
        />
      );
    case 'number':
      return (
        <Textfield
          label={label}
          value={value}
          type="number"
          min={min}
          max={max}
          disabled={isLoading}
          error={errorString}
          onSubmit={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value
            );
            // print type of value
            console.log(typeof value);
            setValue(value);
          }}
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
          disabled={isLoading}
          error={errorString}
          onChange={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value
            );
          }}
        />
      );
    case 'toggle':
      return (
        <ToggleBox
          label={label}
          value={value === 'true'}
          disabled={isLoading}
          error={errorString}
          onChange={async (value) => {
            await axiosPutSingleValue<void>(
              `/instance/${uuid}/game/${setting}`,
              value ? 'true' : 'false'
            );
          }}
        />
      );
  }
}
