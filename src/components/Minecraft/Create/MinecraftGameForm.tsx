import { RadioGroup } from '@headlessui/react';
import clsx from 'clsx';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';

export default function MinecraftAdvancedForm(props: any) {
  const { gameType, setGameType } = props;
  const allGames = [
    { game: 'Minecraft', game_type: 'MinecraftVanilla' },
    { game: 'Minecraft', game_type: 'MinecraftForge' },
    { game: 'Minecraft', game_type: 'MinecraftFabric' },
    { game: 'Minecraft', game_type: 'MinecraftPaper' },
    { game: 'Minecraft', game_type: 'MinecraftSpigot' },
    { game: 'Minecraft', game_type: 'MinecraftBedrock' },
  ];
  console.log(gameType);
  console.log(allGames[0].game_type);
  return (
    <>
      <h1 className="text-h1 font-bold tracking-medium text-gray-300">
        Select Game
      </h1>
      <p>
        Choose your game type.
        <br />
      </p>
      <div className="flex aspect-square flex-wrap">
        {allGames.map((game) => (
          <div
            key={game.game_type}
            className="aspect-square w-full p-2 hover:cursor-pointer sm:w-1/3"
            onClick={() => setGameType(game)}
          >
            <div
              className={clsx(
                game.game_type === gameType.game_type &&
                  'border-2 border-red-300',
                'flex aspect-square items-center justify-center rounded-lg bg-blue-300'
              )}
            >
              <span>{game.game_type}</span>
            </div>
          </div>
        ))}
      </div>
    </>
  );
}
