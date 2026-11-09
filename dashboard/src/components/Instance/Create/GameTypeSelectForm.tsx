import clsx from 'clsx';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { InstanceGameTypes } from 'data/InstanceGameTypes';
import SelectGameCard from './SelectGameCard';
import {
  game_to_description,
  game_to_game_title,
  HandlerGameType_to_Game,
} from 'data/GameTypeMappings';
import { GameInstanceContext } from 'data/GameInstanceContext';
import { useContext } from 'react';
import { useT } from 'i18n';
export default function GameTypeSelectForm({
  onSelectModpack,
  className,
}: {
  /** opens the "create from a Modrinth modpack" flow */
  onSelectModpack: () => void;
  className?: string;
}) {
  const t = useT();
  const { gameType: selectedGameType, setGameType } =
    useContext(GameInstanceContext);
  const { data: game_types, isPending: isLoading } = InstanceGameTypes();
  const heading = (
    <>
      <p className="text-left text-h2 font-extrabold tracking-medium text-gray-300">
        {t('create.gameType.title')}
      </p>
      <p className="text-left text-medium leading-5 font-medium tracking-medium text-white/50 italic">
        {t('create.gameType.subtitle')}
      </p>
    </>
  );
  if (!game_types || isLoading) {
    // the cards' shapes, in place (a full-window spinner used to cover the
    // dialog here)
    return (
      <div className={clsx(className, 'overflow-y-auto p-2')}>
        {heading}
        <SkeletonGroup className="box-border grid grid-cols-2 gap-9 pt-9">
          <Skeleton className="col-span-2 h-32" />
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-32" />
          ))}
        </SkeletonGroup>
      </div>
    );
  }
  return (
    <div className={clsx(className, 'overflow-y-auto p-2')}>
      {heading}
      <div className="box-border grid grid-cols-2 gap-9 pt-9">
        <SelectGameCard
          key="modrinth-modpack"
          title={t('create.gameType.modpack')}
          description={t('create.gameType.modpackDescription')}
          game_type={HandlerGameType_to_Game['MinecraftFabric']}
          badge={t('create.gameType.new')}
          className="col-span-2"
          onClick={onSelectModpack}
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
