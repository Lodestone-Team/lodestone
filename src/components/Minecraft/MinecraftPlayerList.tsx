import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';
import { PlayerListItem, PlayerListCard } from 'components/PlayerListCard';
import { Player } from 'bindings/Player';
// import DashboardCard from 'components/DashboardCard';
// import { useEffect, useState } from 'react';
// import Dashboard from 'pages/dashboard';
// import Label from 'components/Atoms/Label';
// import axios from 'axios';

export default function MinecraftPlayerList() {

  const { selectedInstance: instance } = useContext(InstanceContext);
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
  
  // TODO: Add button to sort usernames alphabetically and reverse
  // const playerList = instance.player_list || [];
  const playerList: Player[] = [
    { type: 'MinecraftPlayer', uuid: '3e5dd54c49ca4742a6c0775c32a2f85e', name: 'koobine' },
    { type: 'MinecraftPlayer', uuid: 'd93dbda4e357471db4e661c59697c651', name: 'sivaniel' },
    { type: 'MinecraftPlayer', uuid: '66aa923f94ae42fba61fbcccd642a4c1', name: 'SpookyCrepe' }
  ];
  
  // ! Debugging purposes, remove later
  console.log('List of players:');
  console.log(playerList);
  
  // API to get the avatar head png 16x16 px
  // TODO: Maybe find some way to handle case where mc-heads.net is down??
  const mcHeadURL = 'https://mc-heads.net/avatar/';
  const avatarDimension = 16;

  return (
    <div>
      <h2 className="text-h3 font-bold tracking-medium">Player List</h2>
      <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
        All players currently online
      </h3>
      <PlayerListCard className="mt-4" >
        {playerList.map((player) => (
          <PlayerListItem key={player.uuid} className="mx-4">
            <img src={`${mcHeadURL}${player.uuid}/${avatarDimension}.png`} alt={`Avatar of ${player.name}`} className="mr-4"/>
            {player.name}
          </PlayerListItem>
        ))}
      </PlayerListCard>
    </div>
  );
}
