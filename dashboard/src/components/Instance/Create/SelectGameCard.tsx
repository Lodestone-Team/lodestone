import GameIcon from 'components/Atoms/GameIcon';
import React from 'react';
import clsx from 'clsx';
import { Game } from 'bindings/Game';
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
        // min-h, not h: a longer translation grows the card, not overflows it
        'min-h-32 w-full rounded-md p-4 outline-1 outline-solid',
        'text-gray-faded/30 enabled:hover:text-white/50 disabled:text-gray-900',
        'border-gray-faded/30 bg-gray-800 enabled:hover:border-gray-faded/50 enabled:active:border-gray-faded/50 enabled:active:bg-gray-800 disabled:border-fade-700/10',
        'outline-gray-faded/30 enabled:hover:outline-white/50',
        'focus-visible:outline-hidden enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        className
      )}
      onClick={onClick}
    >
      <div className="relative">
        <GameIcon game_type={game_type} className="absolute h-6 w-6" />
        <div className="ml-8 text-h3 leading-6 font-mediumbold text-gray-300">
          {title}
        </div>
      </div>
      <div className="mt-2 text-medium leading-5 font-medium text-white/50 italic">
        {description}
      </div>
    </button>
  );
};

export default SelectGameCard;
