import DashboardCard from 'components/DashboardCard';
import Textfield from 'components/Atoms/Config/InputBox';
import { updateInstance } from 'data/InstanceList';
import { axiosPutSingleValue, axiosWrapper, parseintStrict } from 'utils/util';
import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useInstanceManifest } from 'data/InstanceManifest';
import { useGameSetting } from 'data/GameSetting';
import Dropdown from 'components/Atoms/Config/SelectBox';
import SettingField from 'components/SettingField';
import { useContext } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { PortStatus } from 'bindings/PortStatus';

export default function MinecraftGeneralCard() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const queryClient = useQueryClient();
  const { data: manifest, isLoading } = useInstanceManifest(instance.uuid);
  const supportedOptions = manifest?.supported_operations
    ? manifest.supported_operations
    : [];
  const currentSettings = manifest?.settings ? manifest.settings : [];
  const uuid = instance.uuid;

  if (isLoading) {
    // TODO: show an unobtrusive loading screen, reduce UI flicker
    return <div>Loading...</div>;
  }

  const portField = (
    <Textfield
      label="Port"
      value={instance.port.toString()}
      type="number"
      min={0}
      max={65535}
      disabled={!supportedOptions.includes('SetPort')}
      validate={async (port) => {
        const numPort = parseintStrict(port);
        const result = await axiosWrapper<PortStatus>({
          method: 'get',
          url: `/check/port/${numPort}`,
        });
        if (result.is_allocated) throw new Error('Port not available');
      }}
      onSubmit={async (port) => {
        const numPort = parseintStrict(port);
        await axiosPutSingleValue<void>(`/instance/${uuid}/port`, numPort);
        updateInstance(uuid, queryClient, (oldData) => ({
          ...oldData,
          port: numPort,
        }));
      }}
    />
  );

  const maxPlayersField = supportedOptions.includes('GetMaxPlayerCount') ? (
    <Textfield
      label="Max Players"
      value={instance.max_player_count?.toString() ?? ''}
      type="number"
      min={0}
      max={10000}
      // disabled={!supportedOptions.includes('SetMaxPlayers')}
      onSubmit={async (maxPlayers) => {
        const numMaxPlayers = parseintStrict(maxPlayers);
        await axiosPutSingleValue<void>(
          `/instance/${uuid}/players/max`,
          numMaxPlayers
        );
        updateInstance(uuid, queryClient, (oldData) => ({
          ...oldData,
          max_player_count: numMaxPlayers,
        }));
      }}
    />
  ) : null;

  const minRamField = supportedOptions.includes('GetMinRam') ? (
    <Textfield
      label="Min RAM"
      value={instance.min_ram?.toString() ?? ''}
      type="number"
      min={0}
      max={100000}
      disabled={!supportedOptions.includes('SetMinRam')}
      onSubmit={async (minRam) => {
        const numMinRam = parseInt(minRam);
        await axiosPutSingleValue<void>(`/instance/${uuid}/min_ram`, numMinRam);
        updateInstance(uuid, queryClient, (oldData) => ({
          ...oldData,
          min_ram: numMinRam,
        }));
      }}
    />
  ) : null;

  const maxRamField = supportedOptions.includes('GetMaxRam') ? (
    <Textfield
      label="Max RAM"
      value={instance.max_ram?.toString() ?? ''}
      type="number"
      min={0}
      max={100000}
      disabled={!supportedOptions.includes('SetMaxRam')}
      onSubmit={async (maxRam) => {
        const numMaxRam = parseInt(maxRam);
        await axiosPutSingleValue<void>(`/instance/${uuid}/max_ram`, numMaxRam);
        updateInstance(uuid, queryClient, (oldData) => ({
          ...oldData,
          max_ram: numMaxRam,
        }));
      }}
    />
  ) : null;

  const gameModeField = currentSettings.includes('gamemode') ? (
    <SettingField
      instance={instance}
      setting="gamemode"
      label="Game Mode"
      type="dropdown"
      options={['survival', 'creative', 'adventure']}
    />
  ) : null;

  const difficultyField = currentSettings.includes('difficulty') ? (
    <SettingField
      instance={instance}
      setting="difficulty"
      label="Difficulty"
      type="dropdown"
      options={['peaceful', 'easy', 'normal', 'hard']}
    />
  ) : null;

  const onlineModeField = currentSettings.includes('online-mode') ? (
    <SettingField
      instance={instance}
      setting="online-mode"
      label='"Online Mode"'
      type="toggle"
      options={['true', 'false']}
    />
  ) : null;

  const pvpField = currentSettings.includes('pvp') ? (
    <SettingField
      instance={instance}
      setting="pvp"
      label="PvP"
      type="toggle"
      options={['true', 'false']}
    />
  ) : null;

  return (
    <div className="flex flex-col @4xl:flex-row gap-4">
      <div className="w-[28rem]">
        <h1 className="font-black text-large"> General Settings </h1>
        <h2 className="font-medium text-base italic tracking-tight text-white/50">
          Most commonly used settings for your server
        </h2>
      </div>
      <div className="w-full child:w-full rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 last:child:border-b-0 last:child:rounded-b-lg first:child:rounded-t-lg">
        {portField}
        {maxPlayersField}
        {minRamField}
        {maxRamField}
        {gameModeField}
        {difficultyField}
        {onlineModeField}
        {pvpField}
      </div>
    </div>
  );
}
