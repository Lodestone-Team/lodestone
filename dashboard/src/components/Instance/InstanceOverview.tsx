import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import { axiosPutSingleValue } from 'utils/util';
import {
  Fraction,
  MonoText,
  StatusBadge,
  stateTone,
} from 'components/Atoms/Values';
import { CopyButton, FactStrip } from 'components/Values';
import { instanceGamePhrase, instanceLoaderPhrase } from 'utils/values';
import EditableTextfield from 'components/EditableTextfield';
import { useQueryClient } from '@tanstack/react-query';
import InstancePerformance from 'components/Instance/InstancePerformance';
import { InstanceContext } from 'data/InstanceContext';
import GameIcon from 'components/Atoms/GameIcon';
import { useGlobalSettings } from 'data/GlobalSettings';

import { useDocumentTitle } from 'usehooks-ts';
import InstancePlayerList from './InstancePlayerList';
import InstanceDetails from './InstanceDetails';
import InstanceShareSection from './InstanceShareSection';
import { useT } from 'i18n';

const InstanceOverview = () => {
  const t = useT();
  useDocumentTitle(
    t('common.pageTitle', { page: t('server.overview.pageTitle') })
  );
  const { core } = useContext(LodestoneContext);
  const { address } = core;
  const { selectedInstance: instance } = useContext(InstanceContext);
  const { data: globalSettings } = useGlobalSettings();
  const domain = globalSettings?.domain || address || 'localhost';
  const queryClient = useQueryClient();
  const uuid = instance?.uuid;

  if (!instance || !uuid) {
    return (
      <div
        className="@container relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              {t('server.notFound')}
            </h1>
          </div>
        </div>
      </div>
    );
  }

  const loader = instanceLoaderPhrase(
    instance.game_type,
    instance.loader_version
  );
  const joinAddress = `${domain}:${instance.port}`;

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
        className="@container relative flex h-full w-full max-w-2xl flex-col justify-center"
        key={uuid}
      >
        {/* main content container */}
        <div className="flex w-full grow flex-col items-stretch gap-2">
          <div className="flex w-full min-w-0 flex-row items-center gap-4">
            <EditableTextfield
              initialText={instance.name}
              type={'heading'}
              onSubmit={setInstanceName}
              placeholder={t('server.overview.noName')}
              containerClassName="min-w-0"
            />
          </div>
          <div className="-mt-2 flex flex-row flex-wrap items-center gap-2">
            <GameIcon
              game_type={instance.game_type}
              icon_url={instance.icon_url}
              className="h-6 w-6"
            />
            <StatusBadge tone={stateTone(instance.state)}>
              {t(`common.state.${instance.state}`)}
            </StatusBadge>
          </div>
          <FactStrip
            className="mt-1"
            items={[
              {
                label: t('values.key.game'),
                value: instanceGamePhrase(
                  instance.game_type,
                  instance.version || null
                ),
              },
              ...(loader
                ? [{ label: t('values.key.loader'), value: loader }]
                : []),
              {
                label: t('values.key.players'),
                value: (
                  <Fraction
                    done={instance.player_count}
                    total={instance.max_player_count}
                    className="text-gray-300"
                  />
                ),
              },
              {
                label: t('values.key.address'),
                value: (
                  <>
                    <MonoText
                      text={joinAddress}
                      truncate="end"
                      className="text-small font-medium"
                    />
                    <CopyButton value={joinAddress} />
                  </>
                ),
              },
            ]}
          />
          <div className="flex w-full flex-row items-center gap-2">
            <EditableTextfield
              initialText={instance.description}
              type={'description'}
              onSubmit={setInstanceDescription}
              placeholder={t('server.overview.noDescription')}
              containerClassName="min-w-0"
            />
          </div>
        </div>
      </div>
      <InstanceShareSection instance={instance} />
      <InstancePerformance />
      <InstancePlayerList />
      <InstanceDetails instance={instance} />
    </>
  );
};

export default InstanceOverview;
