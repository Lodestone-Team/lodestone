import ClipboardTextfield from 'components/ClipboardTextfield';
import Label from 'components/Atoms/Label';
import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import { axiosPutSingleValue, stateToLabelColor } from 'utils/util';
import EditableTextfield from 'components/EditableTextfield';
import { useQueryClient } from '@tanstack/react-query';
import InstancePerformance from 'components/Instance/InstancePerformance';
import { InstanceContext } from 'data/InstanceContext';
import GameIcon from 'components/Atoms/GameIcon';
import { useGlobalSettings } from 'data/GlobalSettings';

import { useDocumentTitle } from 'usehooks-ts';
import InstancePlayerList from './InstancePlayerList';

const InstanceOverview = () => {
  useDocumentTitle('Instance Overview - Lodestone');
  const { core } = useContext(LodestoneContext);
  const { address } = core;
  const { selectedInstance: instance } = useContext(InstanceContext);
  const { data: globalSettings } = useGlobalSettings();
  const domain = (globalSettings?.domain ?? address) || 'localhost';
  const queryClient = useQueryClient();
  const uuid = instance?.uuid;

  if (!instance || !uuid) {
    return (
      <div
        className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              Instance not found
            </h1>
          </div>
        </div>
      </div>
    );
  }

  const labelColor = stateToLabelColor[instance.state];

  // tablist is map from GameType to tabs

  const setInstanceName = async (name: string) => {
    await axiosPutSingleValue<void>(`/instance/${uuid}/name`, name);
    updateInstance(uuid, queryClient, (oldData) => ({
      ...oldData,
      name,
    }));
  };

  const setInstanceDescription = async (description: string) => {
    await axiosPutSingleValue<void>(
      `/instance/${uuid}/description`,
      description
    );
    updateInstance(uuid, queryClient, (oldData) => ({
      ...oldData,
      description,
    }));
  };

  return (
    <>
      <div
        className="relative flex h-full w-full max-w-2xl flex-col justify-center @container"
        key={uuid}
      >
        {/* main content container */}
        <div className="flex w-full grow flex-col items-stretch gap-2 ">
          <div className="flex w-full min-w-0 flex-row items-center gap-4">
            <EditableTextfield
              initialText={instance.name}
              type={'heading'}
              onSubmit={setInstanceName}
              placeholder="No name"
              containerClassName="min-w-0"
            />
          </div>
          <div className="-mt-2 flex flex-row flex-wrap items-center gap-4">
            <GameIcon game_type={instance.game_type} className="h-6 w-6" />
            <Label size="large" color={labelColor}>
              {instance.state}
            </Label>
            <Label size="large" color={'blue'}>
              Version {instance.version}
            </Label>
            <Label size="large" color={'blue'}>
              Player Count {instance.player_count}/{instance.max_player_count}
            </Label>
            <Label size="large" color={'blue'}>
              <ClipboardTextfield
                text={`${domain}:${instance.port}`}
                color="blue"
                iconLeft={false}
              />
            </Label>
          </div>
          <div className="flex w-full flex-row items-center gap-2">
            <EditableTextfield
              initialText={instance.description}
              type={'description'}
              onSubmit={setInstanceDescription}
              placeholder="No description"
              containerClassName="min-w-0"
            />
          </div>
        </div>
      </div>
      <InstancePerformance />
      <InstancePlayerList />
    </>
  );
};

export default InstanceOverview;
