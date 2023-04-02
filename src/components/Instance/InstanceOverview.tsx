import ClipboardTextfield from 'components/ClipboardTextfield';
import Label from 'components/Atoms/Label';
import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext, useState } from 'react';
import { axiosPutSingleValue, stateToLabelColor } from 'utils/util';
import EditableTextfield from 'components/EditableTextfield';
import { useQueryClient } from '@tanstack/react-query';
import InstancePerformanceCard from 'components/Instance/InstancePerformanceCard';
import { InstanceContext } from 'data/InstanceContext';
import GameIcon from 'components/Atoms/GameIcon';
import { useGlobalSettings } from 'data/GlobalSettings';

import { useDocumentTitle } from 'usehooks-ts';
import InstancePlayerList from './InstancePlayerList';

import { Table, TableColumn, TableRow } from 'components/Table';
import { faTrashCan, faEdit, faSkull } from '@fortawesome/free-solid-svg-icons';
import { ButtonMenuProps } from 'components/ButtonMenu';

const InstanceOverview = () => {
  useDocumentTitle('Dashboard - Lodestone');
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

  const columnsBasic: TableColumn[] = [
    { field: 'name', headerName: 'NAME' },
    { field: 'age', headerName: 'AGE' },
    { field: 'city', headerName: 'CITY' },
  ];

  const rowsBasic: TableRow[] = [
    { id: 1, name: 'John', age: 30, city: 'New York' },
    { id: 2, name: 'Jane', age: 25, city: 'Los Angeles' },
    { id: 3, name: 'Bob', age: 45, city: 'Chicago' },
  ];

  const columnsAnalog: TableColumn[] = [
    { field: 'make', headerName: 'MAKE' },
    { field: 'model', headerName: 'MODEL' },
    { field: 'lens', headerName: 'LENS' },
    { field: 'format', headerName: 'FORMAT' },
    { field: 'year', headerName: 'YEAR' },
  ];
  
  const rowsAnalog: TableRow[] = [
    { id: 1, make: 'Nikon', model: 'FM2', lens: '50mm f/1.8', format: '35mm', year: 1982 },
    { id: 2, make: 'Canon', model: 'AE-1', lens: '50mm f/1.4', format: '35mm', year: 1976 },
    { id: 3, make: 'Pentax', model: 'K1000', lens: '50mm f/2.0', format: '35mm', year: 1976 },
    { id: 4, make: 'Mamiya', model: 'RB67', lens: '127mm f/3.8', format: '120', year: 1970 },
    { id: 5, make: 'Hasselblad', model: '500CM', lens: '80mm f/2.8', format: '120', year: 1957 },
    { id: 6, make: 'Leica', model: 'M6', lens: '35mm f/2.0', format: '35mm', year: 1984 },
    { id: 7, make: 'Fuji', model: 'GW690III', lens: '90mm f/3.5', format: '120', year: 1980 },
    { id: 8, make: 'Minolta', model: 'X-700', lens: '50mm f/1.7', format: '35mm', year: 1981 },
    { id: 9, make: 'Rollei', model: '35T', lens: '40mm f/3.5', format: '35mm', year: 1960 },
    { id: 10, make: 'Kodak', model: 'Retina IIc', lens: '50mm f/2.8', format: '35mm', year: 1954 },
    { id: 11, make: 'Yashica', model: 'Mat-124G', lens: '80mm f/3.5', format: '120', year: 1970 },
    { id: 12, make: 'Voigtlander', model: 'Bessa R3A', lens: '40mm f/1.4', format: '35mm', year: 2004 },
    { id: 13, make: 'Zenza Bronica', model: 'SQ-Ai', lens: '80mm f/2.8', format: '120', year: 1982 },
    { id: 14, make: 'Konica', model: 'Hexar AF', lens: '35mm f/2.0', format: '35mm', year: 1993 },
    { id: 15, make: 'Zeiss Ikon Zeiss Ikon Zeiss Ikon Zeiss Ikon', model: 'Contessa S310', lens: '45mm f/2.8', format: '35mm', year: 1957 },
    { id: 16, make: 'what the fuck am i doing like, the hell i am looooong words', model: 'Instax Mini 9', lens: '60mm f/12.7', format: 'Instant', year: 2017 },
    // { id: 17, make: 'Polaroid', model: 'SX-70', lens: '116mm f/8', format: 'Polaroid', year: 1972 },
    // { id: 18, make: 'Fujifilm', model: 'Instax Mini 90', lens: '60mm f/12.7', format: 'Instant', year: 2013 },
    // { id: 19, make: 'Yashica', model: 'Mat-124G', lens: '80mm f/3.5', format: '120', year: 1970 },
    // { id: 20, make: 'Holga', model: '120N', lens: '60mm f/8', format: '120', year: 1982 },
    // { id: 21, make: 'Kodak', model: 'Brownie Hawkeye', lens: '75mm f/14.5', format: '620', year: 1949 },
    // { id: 22, make: 'Rollei', model: '35', lens: '40mm f/3.5', format: '35mm', year: 1966 },
    // { id: 23, make: 'Agfa', model: 'Clack', lens: '95mm f/11', format: '120', year: 1954 },
    // { id: 24, make: 'Lomography', model: 'Diana F+', lens: '75mm f/8', format: '120', year: 2007 },
    // { id: 25, make: 'Pentax', model: '645', lens: '75mm f/2.8', format: '120', year: 1984 },
    // { id: 26, make: 'Nimslo', model: 'Nimslo 3D', lens: '30mm f/5.6', format: '35mm', year: 1980 },
    // { id: 27, make: 'Voigtlander', model: 'Bessa R2A', lens: '50mm f/1.5', format: '35mm', year: 2004 },
    // { id: 28, make: 'Fujifilm', model: 'GFX 50S', lens: '63mm f/2.8', format: 'Medium Format', year: 2017 },
    // { id: 29, make: 'Bronica', model: 'SQ-A', lens: '80mm f/2.8', format: 'Medium Format', year: 1980 },
    // { id: 30, make: 'Minox', model: '35 EL', lens: '35mm f/2.8', format: '35mm', year: 1974 },
  ];

  const menuItems1: ButtonMenuProps = {
    menuItems: [
      {
        label: 'Edit in file viewer',
        icon: faEdit,
        variant: 'text',
        intention: 'info',
        disabled: false,
        onClick: () => console.log('Button 1 clicked'),
      },
      {
        label: 'another one',
        icon: faSkull,
        variant: 'text',
        intention: 'info',
        disabled: false,
        onClick: () => console.log('Button 1 clicked'),
      },
      {
        label: 'Obliterate',
        icon: faTrashCan,
        variant: 'text',
        intention: 'danger',
        disabled: false,
        onClick: () => console.log('Button 2 clicked'),
      },
    ]
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
      <InstancePerformanceCard />
      <Table rows={rowsBasic} columns={columnsBasic} menuOptions={menuItems1} alignment='even' />
      <Table rows={rowsAnalog} columns={columnsAnalog} menuOptions={menuItems1} alignment='even' />
      <InstancePlayerList />
    </>
  );
};

export default InstanceOverview;
