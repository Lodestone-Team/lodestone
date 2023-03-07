import { HandlerGameType } from 'bindings/HandlerGameType';
import clsx from 'clsx';
import Spinner from 'components/DashboardLayout/Spinner';
import { InstanceGameTypes } from 'data/InstanceGameTypes';
import SelectGameCard from './SelectGameCard';
import { gameTypeInfoFromHandlerType } from 'data/GameTypeMappings';
export default function GameTypeSelectForm({
  gameType,
  setGameType,
  className,
}: {
  gameType: HandlerGameType;
  setGameType: (gameType: HandlerGameType) => void;
  className?: string;
}) {
  const { data: game_types, isLoading, error } = InstanceGameTypes();
  if (!game_types || isLoading) {
    return <Spinner />;
  }

  return (
    <div className={className}>
      <p className="text-left text-h2 font-bold tracking-medium text-gray-300">
        Select Game
      </p>
      <p className="text-left text-medium font-bold italic leading-5 tracking-medium text-white/50">
        What will your server be used for?
      </p>
      <div className="box-border grid grid-cols-2 gap-9 pt-9">
        {game_types.map((game) => {
          const { title, description, game_type } =
            gameTypeInfoFromHandlerType[game];
          return (
            <>
              <SelectGameCard
                key={game}
                title={title}
                description={description}
                game_type={game_type}
                className={clsx(
                  game === gameType &&
                    'enabled:border-gray-faded/50 enabled:bg-gray-700 enabled:outline-white/50'
                )}
                onClick={() => setGameType(game)}
              />
            </>
          );
        })}
      </div>
    </div>
  );
}
