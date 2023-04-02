import GameIcon from 'components/Atoms/GameIcon';
import React, { Dispatch, SetStateAction } from 'react';
import clsx from 'clsx';
import { Game } from 'bindings/Game';
const SelectGameCard = ({
  title,
  description,
  game_type,
  className,
  onClick,
  setUrlValid,
  setUrl,
  selected,
  errorText,
}: {
  title: string;
  description: string;
  game_type: Game;
  className?: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
  setUrlValid: Dispatch<SetStateAction<boolean>>;
  setUrl: (url: string) => void;
  selected: boolean;
  errorText: string;
}) => {
  const backgroundImage = selected
    ? 'linear-gradient(#303338, #303338), radial-gradient(circle at top left, #2AF588, #334675)'
    : 'linear-gradient(#2B2D32, #2B2D32), radial-gradient(circle at top left, #2AF588, #334675)';
  return (
    <button
      type={'button'}
      className={clsx(
        'relative col-span-2 flex flex-col items-start text-left align-top font-sans tracking-medium',
        'h-36 w-full border-2 border-solid border-transparent p-4 outline outline-1',
        'text-gray-faded/30 enabled:hover:text-white/50 disabled:text-gray-900',
        '',
        'outline-gray-faded/30 enabled:hover:outline-white/50',
        'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        className
      )}
      style={{
        // borderImage: 'linear-gradient(to left, #2AF588, #334675) 50',
        border: 'double 2px transparent',
        borderRadius: '0.375rem',
        backgroundImage: backgroundImage,
        backgroundOrigin: 'border-box',
        backgroundClip: 'padding-box, border-box',
      }}
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
      <input
        className={`input-shape input-background input-outlines input-text-style mt-2 w-full ${
          errorText ? 'input-border-error' : 'input-border-normal'
        }`}
        // value={url}
        onChange={(e) => {
          setUrl(e.target.value);
          setUrlValid(true);
        }}
        name={'Url'}
        placeholder={'Enter url'}
      />
      {errorText && (
        <div
          className={`mt-1 whitespace-nowrap text-right font-sans text-small not-italic text-red
        `}
        >
          {errorText || 'Unknown error'}
        </div>
      )}
    </button>
  );
};

export default SelectGameCard;
