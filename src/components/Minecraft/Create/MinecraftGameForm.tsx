import { RadioGroup } from '@headlessui/react';
import clsx from 'clsx';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import {
  InstanceGameTypes,
  SetupInstanceManifest,
} from 'data/InstanceGameTypes';
import SelectGameCard from './SelectGameCard';
export default function MinecraftAdvancedForm({ gameType, setGameType }: any) {
  // // const { gameType, setGameType } = props;
  const { data: game_types, isLoading, error } = InstanceGameTypes();
  // console.log(gametypes);
  // const {
  //   data: setup_manifest,
  //   isLoading,
  //   error,
  // } = SetupInstanceManifest('MinecraftPaper');
  // console.log(setup_manifest);
  // const allGames = [
  //   {
  //     game: 'Minecraft',
  //     game_type: 'MinecraftVanilla',
  //     description: 'Vanilla Minecraft',
  //   },
  //   {
  //     game: 'Minecraft',
  //     game_type: 'MinecraftForge',
  //     description: 'Minecraft with Forge',
  //   },
  //   {
  //     game: 'Minecraft',
  //     game_type: 'MinecraftFabric',
  //     description: 'Minecraft with Fabric',
  //   },
  //   {
  //     game: 'Minecraft',
  //     game_type: 'MinecraftPaper',
  //     description: 'Minecraft with Paper',
  //   },
  //   {
  //     game: 'Minecraft',
  //     game_type: 'MinecraftSpigot',
  //     description: 'Minecraft with Spigot',
  //   },
  //   {
  //     game: 'Minecraft',
  //     game_type: 'MinecraftBedrock',
  //     description: 'Minecraft Bedrock',
  //   },
  // ];
  // console.log(gameType);
  // console.log(allGames[0].game_type);
  if (!game_types || isLoading) {
    return <div>Loading...</div>;
  }
  return (
    <>
      <h1 className="text-h1 font-bold tracking-medium text-gray-300">
        Select Game
      </h1>
      <p>
        Choose your game type.
        <br />
      </p>
      <div className="flex flex-wrap">
        {game_types.map((game) => (
          <div
            key={game}
            className="w-full p-4 hover:cursor-pointer sm:w-1/2"
            onClick={() => setGameType(game)}
          >
            <SelectGameCard
              title={game}
              description={game}
              className={clsx(
                game === gameType && 'border-2 border-red-300',
                'flex h-28 items-center justify-center rounded-lg bg-blue-300'
              )}
            />
            {/* <div
              className={}
            >
              <span>{game.game_type}</span>
            </div> */}
          </div>
        ))}
      </div>
    </>
  );
}
