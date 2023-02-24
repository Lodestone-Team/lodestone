import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';
import { PlayerListItem, PlayerListCard } from 'components/PlayerListCard';
import { Player } from 'bindings/Player';
import { useEffect, useState } from 'react';
// import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
// import DashboardCard from 'components/DashboardCard';
// import Dashboard from 'pages/dashboard';
// import Label from 'components/Atoms/Label';
// import axios from 'axios';

export default function MinecraftPlayerList() {

  const { selectedInstance: instance } = useContext(InstanceContext);
  const uuid = instance?.uuid;
  
  // JUST FOR TESTING
  const playerListTest: Player[] = [
    { type: 'MinecraftPlayer', uuid: 'd93dbda4e357471db4e661c59697c651', name: 'sivaniel' },
    { type: 'MinecraftPlayer', uuid: '3e5dd54c49ca4742a6c0775c32a2f85e', name: 'koobine' },
    { type: 'MinecraftPlayer', uuid: '13bd048a57f44f1a9ad21d1a3912f178', name: 'Ynng' },
    { type: 'MinecraftPlayer', uuid: 'a988c96a358d4dc9b3435eb01c0bedca', name: 'arcslogger' },
    { type: 'MinecraftPlayer', uuid: '8c6d087acb204b30a8ef87da748a963e', name: 'IStoleYourSocks' },
    { type: 'MinecraftPlayer', uuid: 'c709ad04cf80493581db9bf715c92733', name: '3nj' },
    { type: 'MinecraftPlayer', uuid: 'c1d2e3157c3a4a978a8effd188cad54d', name: 'PocketSage' },
    { type: 'MinecraftPlayer', uuid: 'b65635533dff4ed0a4e6c24032fc146e', name: 'Kevin1' },
    { type: 'MinecraftPlayer', uuid: 'e0688b364e92478fb8b7fcabcb811f7e', name: 'kevin2' },
    { type: 'MinecraftPlayer', uuid: '66aa923f94ae42fba61fbcccd642a4c1', name: 'SpookyCrepe' }
  ];

  // Default value of playerList will be an empty array since we cannot have useState after a conditional block
  // const playerListDefault: Player[] = [];
  const [playerList, setPlayerList] = useState([
    { type: 'MinecraftPlayer', uuid: '', name: ''}
  ]);

  useEffect(() => {
    console.log('changed detected');
    if (instance && instance.player_list) {
      setPlayerList(instance.player_list);
    }
    console.log(instance?.player_count);
    console.log(instance?.player_list);
  }, [instance, playerList])

  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');
  
  const handleSortClick = () => {
    // Sort the player list in alphabetical order by name
    const playerListSorted = [...playerList].sort((a, b) =>
      a.name.localeCompare(b.name)
    );
  
    // If the current sort order is descending, reverse the sorted array
    if (sortOrder === 'desc') {
      playerListSorted.reverse();
    }
  
    // Toggle the sort order for the next click
    setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc');
  
    // Update the player list state with the sorted array
    setPlayerList(playerListSorted);
  };
  
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
  
  // const playerList = instance.player_list || [];
  
  // API to get the avatar head png 16x16 px
  const mcHeadURL = 'https://mc-heads.net/avatar/';
  const avatarDimension = 16;

  return (
    <div>
      <h2 className="text-h3 font-bold tracking-medium">Player List</h2>
      <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
        All players currently online
      </h3>
      <button
        className="flex items-center justify-center text-h3 font-medium tracking-medium text-white/50"
        onClick={handleSortClick}
      >
        NAME
      </button>
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
