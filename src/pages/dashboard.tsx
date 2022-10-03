import { faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Tab } from '@headlessui/react';
import ClipboardTextfield from 'components/ClipboardTextfield';
import GameConsole from 'components/GameConsole';
import DashboardCard from 'components/DashboardCard';
import DashboardLayout from 'components/DashboardLayout';
import Label from 'components/Label';
import {
  InstanceInfo,
  updateInstance,
  useInstanceList,
} from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useRouter } from 'next/router';
import {
  ReactElement,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useRouterQuery } from 'utils/hooks';
import { isAxiosError, pushKeepQuery, stateToLabelColor } from 'utils/util';
import { NextPageWithLayout } from './_app';
import EditableTextfield from 'components/EditableTextfield';
import axios, { AxiosError } from 'axios';
import { useQueryClient } from '@tanstack/react-query';
import { ClientError } from 'data/ClientError';

const Dashboard: NextPageWithLayout = () => {
  const lodestoneContex = useContext(LodestoneContext);
  const { query: uuid } = useRouterQuery('uuid');
  const { data: instances } = useInstanceList();
  const queryClient = useQueryClient();

  const instance = useMemo(() => {
    if (uuid) return instances?.[uuid];
  }, [uuid, instances]);

  // TODO: add loading state, don't let it flash blank
  if (!uuid) return <></>;

  if (!instance) {
    return (
      <div className="px-12 py-10 bg-gray-800">
        <h1 className="-ml-4 font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
          Instance not found
        </h1>
      </div>
    );
  }

  const labelColor = stateToLabelColor[instance.state];

  const tabList = ['Console', 'Placeholder'];

  const setInstanceName = async (name: string) => {
    try {
      await axios({
        method: 'put',
        url: `/instance/${uuid}/name`,
        headers: {
          'Content-Type': 'application/json',
        },
        data: JSON.stringify(name),
      });
      updateInstance(uuid, queryClient, (oldData) => ({
        ...oldData,
        name,
      }));
      return { error: false, message: '' };
    } catch (error) {
      console.error(error);
      if (isAxiosError<ClientError>(error))
        if (error.response?.data)
          return { error: true, message: error.response.data.detail };
        else return { error: true, message: error.message };
      return { error: true, message: 'Unknown error' };
    }
  };

  const setInstanceDescription = async (description: string) => {
    try {
      await axios({
        method: 'put',
        url: `/instance/${uuid}/description`,
        headers: {
          'Content-Type': 'application/json',
        },
        data: JSON.stringify(description),
      });
      updateInstance(uuid, queryClient, (oldData) => ({
        ...oldData,
        description,
      }));
      return { error: false, message: '' };
    } catch (error) {
      console.error(error);
      if (isAxiosError<ClientError>(error))
        if (error.response?.data)
          return { error: true, message: error.response.data.detail };
        else return { error: true, message: error.message };
      return { error: true, message: 'Unknown error' };
    }
  };

  return (
    <div
      className="h-0 px-12 pt-6 pb-10 overflow-y-auto bg-gray-800 grow"
      key={uuid}
    >
      <div className="flex flex-col items-start h-full gap-2">
        <div className="flex flex-row items-center w-full gap-12">
          <div className="flex flex-row items-center min-w-0 gap-4">
            {/* TODO: create a universal "text with edit button" component */}
            <EditableTextfield
              initialText={instance.name}
              type={'heading'}
              onSubmit={setInstanceName}
              placeholder="No name"
              containerClassName="min-w-0"
            />
          </div>
          <div className="flex flex-row items-center gap-4">
            {/* TODO: create a universal game flavour image component */}
            <img
              src="/assets/minecraft-vanilla.png"
              alt={`${instance.game_type} logo`}
              className="w-8 h-8"
            />
            <Label size="large" color={labelColor}>
              {instance.state}
            </Label>
          </div>
        </div>
        <div className="flex flex-row items-center gap-4 -mt-2">
          <Label size="large" color={labelColor}>
            Player Count {instance.player_count}/{instance.max_player_count}
          </Label>
          <Label
            size="large"
            color="gray"
            className="flex flex-row items-center gap-3"
          >
            <ClipboardTextfield
              text={`${lodestoneContex.address}:${instance.port}`}
              textToCopy={lodestoneContex.address}
            />
          </Label>
        </div>
        <div className="flex flex-row items-center w-full gap-2">
          {/* TODO: create a universal "text with edit button" component */}
          <EditableTextfield
            initialText={instance.description}
            type={'description'}
            onSubmit={setInstanceDescription}
            placeholder="No description"
            containerClassName="min-w-0"
          />
        </div>
        <Tab.Group>
          <Tab.List className="flex flex-row items-center w-full gap-4 mb-4 border-b-2 border-gray-700">
            {tabList.map((tab) => (
              <Tab
                key={tab}
                className={({ selected }) =>
                  `tracking-tight text-medium font-semibold focus-visible:outline-none ${
                    selected
                      ? 'text-blue border-b-2 border-blue'
                      : 'text-gray-500 mb-0.5'
                  }`
                }
              >
                {tab}
              </Tab>
            ))}
          </Tab.List>
          <Tab.Panels className="w-full grow">
            <Tab.Panel className="w-full h-full">
              {/* <h1 className="font-bold text-medium"> Console </h1> */}
              <GameConsole
                uuid={uuid}
                enableInput={instance.state === 'Running'}
              />
            </Tab.Panel>
            <Tab.Panel>
              <DashboardCard>
                <h1 className="font-bold text-medium"> Placeholder </h1>
              </DashboardCard>
            </Tab.Panel>
          </Tab.Panels>
        </Tab.Group>
      </div>
    </div>
  );
};

Dashboard.getLayout = (page: ReactElement): ReactNode => (
  <DashboardLayout>{page}</DashboardLayout>
);

export default Dashboard;
