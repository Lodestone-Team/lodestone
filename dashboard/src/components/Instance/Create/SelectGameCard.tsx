import GameIcon from 'components/Atoms/GameIcon';
import React from 'react';
import clsx from 'clsx';
import { Game } from '@bindings/Game';
const SelectGameCard = ({
  title,
  description,
  game_type,
  className,
  onClick,
}: {
  title: string;
  description: string;
  game_type: Game;
  className?: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
}) => {
  return (
    <button
      type={'button'}
      className={clsx(
        'flex flex-col items-start text-left align-top font-sans tracking-medium',
        'h-32 w-full rounded-md p-4 outline outline-1',
        'text-gray-faded/30 enabled:hover:text-white/50 disabled:text-gray-900',
        'border-gray-faded/30 bg-gray-800 enabled:hover:border-gray-faded/50  enabled:active:border-gray-faded/50 enabled:active:bg-gray-800 disabled:border-fade-700/10',
        'outline-gray-faded/30 enabled:hover:outline-white/50',
        'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        className
      )}
      onClick={onClick}
    >
      <div className="relative">
        <GameIcon game_type={game_type} className="absolute h-6 w-6" />
        <div className="ml-8 text-h3 font-mediumbold leading-6 text-gray-300">
          {title}
        </div>
      </div>
      <div className="mt-2 text-medium font-medium italic leading-5 text-white/50">
        {description}
      </div>
    </button>
  );
};

export default SelectGameCard;
