import GameIcon from 'components/Atoms/GameIcon';
import React from 'react';
import clsx from 'clsx';
import { GameType } from 'bindings/InstanceInfo';
const SelectGameCard = ({
  title,
  description,
  game_type,
  className,
  onClick,
}: {
  title: string;
  description: string;
  game_type: GameType;
  className?: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
}) => {
  return (
    <button
      type={'button'}
      className={clsx(
        className,
        'flex flex-col items-start text-left align-top font-sans tracking-medium'
      )}
      onClick={onClick}
    >
      <div className="relative">
        <GameIcon game_type={game_type} className="absolute h-6 w-6" />
        <div className="ml-8 text-h3 font-extrabold leading-6 text-gray-300">
          {title}
        </div>
      </div>
      <div className="text-medium font-mediumbold italic leading-5 text-white/50">
        {description}
      </div>
    </button>
  );
};

export default SelectGameCard;
