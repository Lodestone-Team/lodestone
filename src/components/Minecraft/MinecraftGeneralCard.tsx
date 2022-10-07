import DashboardCard from 'components/DashboardCard';
import Textfield from 'components/Textfield';
import { updateInstance } from 'data/InstanceList';
import { axiosPutSingleValue, axiosWrapper } from 'utils/util';
import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useInstanceManifest } from 'data/InstanceManifest';

export default function MinecraftGeneralCard({
  instance,
}: {
  instance: InstanceInfo;
}) {
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
      removeArrows={true}
      min={0}
      max={65535}
      disabled={!supportedOptions.includes('SetPort')}
      validate={async (port) => {
        const numPort = parseInt(port);
        const result = await axiosWrapper<boolean>({
          method: 'get',
          url: `/check/port/${numPort}`,
        });
        if (result) throw new Error('Port not available');
      }}
      onSubmit={async (port) => {
        const numPort = parseInt(port);
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
      removeArrows={true}
      // disabled={!supportedOptions.includes('SetMaxPlayers')}
      onSubmit={async (maxPlayers) => {
        const numMaxPlayers = parseInt(maxPlayers);
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
      removeArrows={true}
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
      removeArrows={true}
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

  return (
    <DashboardCard>
      <h1 className="font-bold text-medium"> General Settings </h1>
      <div className="grid w-full grid-cols-2 gap-4 md:grid-cols-4 child:w-full">
        {portField}
        {maxPlayersField}
        {minRamField}
        {maxRamField}
      </div>
    </DashboardCard>
  );
}
