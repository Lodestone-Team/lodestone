import DashboardCard from 'components/DashboardCard';
import InputBox from 'components/Atoms/Config/InputBox';
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
    <InputBox
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
      descriptionFunc={(port) => `Server will be available on port ${port}`}
    />
  );

  const maxPlayersField = supportedOptions.includes('GetMaxPlayerCount') ? (
    <InputBox
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
      descriptionFunc={(maxPlayers) =>
        `A maximum of ${maxPlayers} players can connect`
      }
    />
  ) : null;

  const minRamField = supportedOptions.includes('GetMinRam') ? (
    <InputBox
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
      descriptionFunc={(minRam) =>
        `Minimum RAM allocated to the server: ${minRam} MB`
      }
    />
  ) : null;

  const maxRamField = supportedOptions.includes('GetMaxRam') ? (
    <InputBox
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
      descriptionFunc={(maxRam) =>
        `Maximum RAM allocated to the server: ${maxRam} MB`
      }
    />
  ) : null;

  return (
    <div className="flex flex-col gap-4 @4xl:flex-row">
      <div className="w-[28rem]">
        <h1 className="text-large font-black"> Server Settings </h1>
        <h2 className="text-base font-medium italic tracking-tight text-white/50">
          Technical settings for the server
        </h2>
      </div>
      <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {portField}
        {maxPlayersField}
        {minRamField}
        {maxRamField}
      </div>
    </div>
  );
}
