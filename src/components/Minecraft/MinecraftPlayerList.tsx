import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';
import { PlayerListItem, PlayerListCard } from 'components/PlayerListCard';
import { useState, useMemo } from 'react';
import { faArrowDown, faArrowUp } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
// import { Player } from 'bindings/Player';

export default function MinecraftPlayerList() {

  const { selectedInstance: instance } = useContext(InstanceContext);
  const uuid = instance?.uuid;
  
  // Ascending order is referring to alphabetical order
  const [sortAscending, setSortAscending] = useState(true);
  
  // Update playerList every time there a player leaves or joins
  // Also sorts and updates when sort button is pressed
  const updatedPlayerList = useMemo(() => {
    if (!instance?.player_list) return null;
    const playerList = [...instance.player_list];
    playerList.sort((a, b) => {
      if (sortAscending) {
        return a.name.localeCompare(b.name);
      } else {
        return b.name.localeCompare(a.name);
      }
    });
    return playerList;
  }, [instance, sortAscending]);
  
  // Catch case where server instance is not available; return early
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
  const avatarDimension = 16;

  return (
    <div>
      <h2 className="text-h3 font-extrabold tracking-medium">Player List</h2>
      {updatedPlayerList && updatedPlayerList.length ? (
        <>
          <h3 className="text-medium font-medium italic tracking-medium text-white/50">
            All players currently online
          </h3>
          <button
            className="mt-4 mb-2 flex items-center justify-center text-small font-medium tracking-medium text-white/50"
            onClick={() => setSortAscending(!sortAscending)}
          >
            NAME
            {sortAscending ? (
              <FontAwesomeIcon icon={faArrowDown} className="mx-1.5" />
              ) : (
                <FontAwesomeIcon icon={faArrowUp} className="mx-1.5" />
            )}
          </button>
          {updatedPlayerList.length > 0 && (
            <PlayerListCard>
              {updatedPlayerList.map((player) => (
                <PlayerListItem key={player.uuid}>
                  <img
                    src={`${mcHeadURL}${player.uuid}/${avatarDimension}.png`}
                    alt={`Avatar of ${player.name}`}
                    className={`mx-1 h-4 w-4`}
                    draggable="false"
                    style={{ imageRendering: "pixelated", userSelect: 'none' }}
                  />
                  <div className="mx-1 text-medium" >
                    {player.name}
                  </div>
                </PlayerListItem>
              ))}
            </PlayerListCard>
          )}
        </>
      ) : (
        <h3 className="text-medium font-medium italic tracking-medium text-white/50">
          No players online
        </h3>
      )}
    </div>
  );
}
