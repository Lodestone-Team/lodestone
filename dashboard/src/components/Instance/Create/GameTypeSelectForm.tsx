import clsx from 'clsx';
import Spinner from 'components/DashboardLayout/Spinner';
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
  className,
}: {
  manifestLoading: boolean;
  manifestError: boolean;
  className?: string;
}) {
  const { gameType: selectedGameType, setGameType } =
    useContext(GameInstanceContext);
  const { data: game_types, isLoading } = InstanceGameTypes();
  if (!game_types || isLoading) {
    return <Spinner />;
  }
  return (
    <div className={clsx(className, ' overflow-y-auto p-2')}>
      <p className="text-left text-h2 font-extrabold tracking-medium text-gray-300">
        Select a game
      </p>
      <p className="text-left text-medium font-medium italic leading-5 tracking-medium text-white/50">
        What will your instance be used for?
      </p>
      <div className="box-border grid grid-cols-2 gap-9 pt-9">
        <SelectGenericGameCard
          key={'Generic'}
          title={'Lodestone Atom'}
          description={'Enter the URL to the Lodestone Atom below:'}
          game_type={{
            type: 'Generic',
            game_name: 'Generic',
            game_display_name: 'Generic',
          }}
          className={clsx(
            'Generic' === selectedGameType &&
              'enabled:generic-gametype-selected enabled:hover:generic-gametype-selected '
          )}
          onClick={() => setGameType('Generic')}
          manifestLoading={manifestLoading}
          errorText={manifestError ? 'Error fetching the instance' : ''}
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
      </div>
    </div>
  );
}
