import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';
import { PlayerListItem, PlayerListCard } from 'components/PlayerListCard';
import { useEffect, useState } from 'react';
import { faArrowDown, faArrowUp } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
// import { Player } from 'bindings/Player';

export default function MinecraftPlayerList() {

  const { selectedInstance: instance } = useContext(InstanceContext);
  const uuid = instance?.uuid;
  
  // JUST FOR TESTING
  // const playerListTest: Player[] = [
  //   { type: 'MinecraftPlayer', uuid: 'd93dbda4e357471db4e661c59697c651', name: 'sivaniel' },
  //   { type: 'MinecraftPlayer', uuid: '3e5dd54c49ca4742a6c0775c32a2f85e', name: 'koobine' },
  //   { type: 'MinecraftPlayer', uuid: '13bd048a57f44f1a9ad21d1a3912f178', name: 'Ynng' },
  //   { type: 'MinecraftPlayer', uuid: 'a988c96a358d4dc9b3435eb01c0bedca', name: 'arcslogger' },
  //   { type: 'MinecraftPlayer', uuid: '8c6d087acb204b30a8ef87da748a963e', name: 'IStoleYourSocks' },
  //   { type: 'MinecraftPlayer', uuid: 'c709ad04cf80493581db9bf715c92733', name: '3nj' },
  //   { type: 'MinecraftPlayer', uuid: 'c1d2e3157c3a4a978a8effd188cad54d', name: 'PocketSage' },
  //   { type: 'MinecraftPlayer', uuid: 'b65635533dff4ed0a4e6c24032fc146e', name: 'Kevin1' },
  //   { type: 'MinecraftPlayer', uuid: 'e0688b364e92478fb8b7fcabcb811f7e', name: 'kevin2' },
  //   { type: 'MinecraftPlayer', uuid: '66aa923f94ae42fba61fbcccd642a4c1', name: 'SpookyCrepe' }
  // ];

  // Default value of playerList will be an empty array since we cannot have useState after a conditional block
  const [playerList, setPlayerList] = useState([
    { type: 'MinecraftPlayer', uuid: '', name: ''}
  ]);
  
  // Ascending order is referring to alphabetical order
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');

  // Update playerList every time there a player leaves or joins
  useEffect(() => {
    console.log('changed detected from useEffect');
    if (instance && instance.player_list) {
      setPlayerList(instance.player_list);
    }
  }, [instance])
  
  // Toggle between sorting alphabetically or reverse alphabetically
  const handleSortClick = () => {
    const playerListSorted = [...playerList].sort((a, b) =>
      a.name.localeCompare(b.name)
    );

    if (sortOrder === 'desc') {
      playerListSorted.reverse();
    }

    setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc');
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
  
  // API to get the avatar head png 16x16 px
  const mcHeadURL = 'https://mc-heads.net/avatar/';
  const avatarDimension = 128;

  return (
    <div>
      <h2 className="text-h3 font-extrabold tracking-medium">Player List</h2>
      {playerList.length ? (
        <>
          <h3 className="text-medium font-medium italic tracking-medium text-white/50">
            All players currently online
          </h3>
          <button
            className="mt-4 mb-2 flex items-center justify-center text-small font-medium tracking-medium text-white/50"
            onClick={handleSortClick}
          >
            NAME
            {sortOrder === 'asc' ? (
              <FontAwesomeIcon icon={faArrowDown} className="mx-1.5" />
              ) : (
                <FontAwesomeIcon icon={faArrowUp} className="mx-1.5" />
            )}
          </button>
        </>
      ) : (
        <h3 className="text-medium font-medium italic tracking-medium text-white/50">
          No players online
        </h3>
      )}
      {playerList.length > 0 && (
        <PlayerListCard>
          {playerList.map((player) => (
            <PlayerListItem key={player.uuid}>
              <img
                src={`${mcHeadURL}${player.uuid}/${avatarDimension}.png`}
                alt={`Avatar of ${player.name}`}
                className="mx-1 h-4 w-4"
              />
              <div className="mx-1 text-medium" >
                {player.name}
              </div>
            </PlayerListItem>
          ))}
        </PlayerListCard>
      )}
    </div>
  );
}
