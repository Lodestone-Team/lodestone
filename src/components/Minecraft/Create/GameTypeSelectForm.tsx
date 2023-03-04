import { RadioGroup } from '@headlessui/react';
import { HandlerGameType } from 'bindings/HandlerGameType';
import { GameType } from 'bindings/InstanceInfo';
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
      <div className="mt-9 box-border grid h-full grid-cols-2 gap-9">
        {game_types.map((game) => {
          const { title, description, game_type } =
            gameTypeInfoFromHandlerType[game];
          return (
            <div key={game}>
              <SelectGameCard
                title={title}
                description={description}
                game_type={game_type}
                className={clsx(
                  'h-28 w-full rounded-md p-4 outline outline-1',
                  'text-gray-faded/30 enabled:hover:text-white/50 disabled:text-gray-900',
                  'border-gray-faded/30 bg-gray-800 enabled:hover:border-gray-faded/50 enabled:hover:bg-gray-700 enabled:active:border-gray-faded/50 enabled:active:bg-gray-800 disabled:border-fade-700/10',
                  'outline-gray-faded/30 enabled:hover:outline-white/50',
                  'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
                  game === gameType &&
                    'enabled:border-gray-faded/50 enabled:bg-gray-700 enabled:outline-white/50'
                )}
                onClick={() => setGameType(game)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
