import { HandlerGameType } from 'bindings/HandlerGameType';
import clsx from 'clsx';
import Spinner from 'components/DashboardLayout/Spinner';
import { InstanceGameTypes } from 'data/InstanceGameTypes';
import SelectGameCard from './SelectGameCard';
import {
  game_to_description,
  game_to_game_title,
  HandlerGameType_to_Game,
} from 'data/GameTypeMappings';
export default function GameTypeSelectForm({
  selectedGameType,
  setGameType,
  className,
}: {
  selectedGameType: HandlerGameType;
  setGameType: (gameType: HandlerGameType) => void;
  className?: string;
}) {
  const { data: game_types, isLoading, error } = InstanceGameTypes();
  if (!game_types || isLoading) {
    return <Spinner />;
  }

  return (
    <div className={className}>
      <p className="text-left text-h2 font-extrabold tracking-medium text-gray-300">
        Select a game
      </p>
      <p className="text-left text-medium font-medium italic leading-5 tracking-medium text-white/50">
        What will your instance be used for?
      </p>
      <div className="box-border grid grid-cols-2 gap-9 pt-9">
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
