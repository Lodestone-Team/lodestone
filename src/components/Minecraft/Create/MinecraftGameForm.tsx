import { RadioGroup } from '@headlessui/react';
import { HandlerGameType } from 'bindings/HandlerGameType';
import { GameType } from 'bindings/InstanceInfo';
import clsx from 'clsx';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import Spinner from 'components/DashboardLayout/Spinner';
import {
  InstanceGameTypes,
  SetupInstanceManifest,
} from 'data/InstanceGameTypes';
import SelectGameCard from './SelectGameCard';
import { gameTypeInfoFromHandlerType } from 'data/GameTypeMappings';
export default function MinecraftAdvancedForm({
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

  const game_typesDuped = [game_types].flat();
  return (
    <div className={className}>
      <p className="text-left text-h2 font-bold tracking-medium text-gray-300">
        Select Game
      </p>
      <p className="text-left text-medium font-bold italic leading-5 tracking-medium text-white/50">
        What will your server be used for?
      </p>
      <div className="mt-9 box-border grid h-full grid-cols-2 gap-9">
        {game_typesDuped.map((game) => {
          const { title, description, game_type } =
            gameTypeInfoFromHandlerType[game];
          console.log(gameTypeInfoFromHandlerType[game]);
          return (
            <div
              key={game}
              className="w-full hover:cursor-pointer"
              onClick={() => setGameType(game)}
            >
              <SelectGameCard
                title={title}
                description={description}
                game_type={game_type}
                className={clsx(
                  'h-28 rounded-lg bg-gray-800 p-4 outline outline-1 outline-fade-700/10',
                  game === gameType && 'border-2 border-red-300'
                )}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
