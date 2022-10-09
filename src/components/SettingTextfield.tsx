// a wrapper around TextField that fetches a single setting from the server

import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useGameSetting } from 'data/GameSetting';
import { useInstanceManifest } from 'data/InstanceManifest';
import { axiosPutSingleValue, errorToMessage } from 'utils/util';
import Textfield from './Textfield';

export default function SettingTextfield({
  instance,
  settingName,
  label,
}: {
  instance: InstanceInfo;
  settingName: string;
  label?: string;
}) {
  const uuid = instance.uuid;
  const {
    data: settingValue,
    isLoading,
    error,
  } = useGameSetting(uuid, settingName);
  label = label ?? settingName;
  const value = settingValue ?? '';

  const errorString = errorToMessage(error);

  return (
    <Textfield
      label={label}
      value={value}
      disabled={isLoading}
      error={errorString}
      onSubmit={async (value) => {
        await axiosPutSingleValue<void>(
          `/instance/${uuid}/game/${settingName}`,
          value
        );
      }}
    />
  );
}
