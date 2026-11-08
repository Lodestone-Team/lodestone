import clsx from 'clsx';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { InstanceGameTypes } from 'data/InstanceGameTypes';
import SelectGameCard from './SelectGameCard';
import {
  game_to_description,
  game_to_game_title,
  HandlerGameType_to_Game,
} from 'data/GameTypeMappings';
import SelectGenericGameCard from './SelectGenericGameCard';
import { GameInstanceContext } from 'data/GameInstanceContext';
import { useContext } from 'react';
export default function GameTypeSelectForm({
  manifestLoading,
  manifestError,
  onSelectModpack,
  className,
}: {
  manifestLoading: boolean;
  manifestError: boolean;
  /** opens the "create from a Modrinth modpack" flow */
  onSelectModpack: () => void;
  className?: string;
}) {
  const { gameType: selectedGameType, setGameType } =
    useContext(GameInstanceContext);
  const { data: game_types, isPending: isLoading } = InstanceGameTypes();
  const heading = (
    <>
      <p className="text-left text-h2 font-extrabold tracking-medium text-gray-300">
        Select a game
      </p>
      <p className="text-left text-medium font-medium italic leading-5 tracking-medium text-white/50">
        What will your server be used for?
      </p>
    </>
  );
  if (!game_types || isLoading) {
    // the cards' shapes, in place (a full-window spinner used to cover the
    // dialog here)
    return (
      <div className={clsx(className, ' overflow-y-auto p-2')}>
        {heading}
        <SkeletonGroup className="box-border grid grid-cols-2 gap-9 pt-9">
          <Skeleton className="col-span-2 h-14" />
          {Array.from({ length: 5 }, (_, i) => (
            <Skeleton key={i} className="h-32" />
          ))}
        </SkeletonGroup>
      </div>
    );
  }
  return (
    <div className={clsx(className, ' overflow-y-auto p-2')}>
      {heading}
      <div className="box-border grid grid-cols-2 gap-9 pt-9">
        <SelectGenericGameCard
          key={'Generic'}
          title={'Learn about Docker Server (beta)'}
          game_type={{
            type: 'Generic',
            game_name: 'Generic',
            game_display_name: 'Docker Server',
          }}
          className={clsx(
            'Generic' === selectedGameType &&
              'enabled:generic-gametype-selected enabled:hover:generic-gametype-selected '
          )}
          onClick={() => setGameType('Generic')}
          manifestLoading={manifestLoading}
          errorText={manifestError ? 'Error fetching the server' : ''}
        />
        {game_types.map((game_type) => {
          const game = HandlerGameType_to_Game[game_type];
          return (
            <SelectGameCard
              key={game_type}
              title={game_to_game_title(game)}
              description={game_to_description(game)}
              game_type={game}
              className={clsx(
                game_type === selectedGameType &&
                  'enabled:border-gray-faded/50 enabled:bg-gray-700 enabled:outline-white/50'
              )}
              onClick={() => setGameType(game_type)}
            />
          );
        })}
        <SelectGameCard
          key="modrinth-modpack"
          title="Modrinth modpack"
          description="Create a Fabric server from a Modrinth modpack"
          game_type={HandlerGameType_to_Game['MinecraftFabric']}
          onClick={onSelectModpack}
        />
      </div>
    </div>
  );
}
