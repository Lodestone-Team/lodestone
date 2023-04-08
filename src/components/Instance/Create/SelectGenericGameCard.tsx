import GameIcon from 'components/Atoms/GameIcon';
import React, { useContext } from 'react';
import clsx from 'clsx';
import { Game } from 'bindings/Game';
import Button from 'components/Atoms/Button';
import { GenericHandlerGameType } from '../InstanceCreateForm';
import { GameInstanceContext } from 'data/GameInstanceContext';
const SelectGenericGameCard = ({
  title,
  description,
  game_type,
  className,
  onClick,
  manifestLoading,
  errorText,
}: {
  title: string;
  description: string;
  game_type: Game;
  className?: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
  manifestLoading: boolean;
  errorText: string;
}) => {
  const {
    gameType: selectedGameType,
    setGameType,
    setUrl,
    urlValid,
    setUrlValid,
    genericFetchReady,
    setGenericFetchReady,
  } = useContext(GameInstanceContext);

  const buttonStatus = genericFetchReady
    ? manifestLoading
      ? 'Fetching...'
      : urlValid
      ? 'Loaded!'
      : 'Load Instance'
    : 'Load Instance';
  return (
    <button
      type={'button'}
      className={clsx(
        'relative col-span-2 flex flex-col items-start text-left align-top font-sans tracking-medium',
        'h-36 w-full border-solid p-4 outline outline-1',
        'generic-gametype-unselected enabled:hover:generic-gametype-hover rounded-md border-gray-faded/30 text-gray-faded/30 disabled:text-gray-900',
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
      <div className="mt-2 mb-1 text-medium font-medium italic leading-5 text-white/50">
        {description}
      </div>
      <div className="flex w-full items-center gap-3">
        <input
          className={`input-shape input-background input-outlines input-text-style h-full w-[70%] ${
            errorText ? 'input-border-error' : 'input-border-normal'
          }`}
          onChange={(e) => {
            if (selectedGameType !== 'Generic') setGameType('Generic');
            setUrl(e.target.value);
            setGenericFetchReady(false);
            setUrlValid(false);
          }}
          name={'Url'}
          placeholder={'Enter url'}
          disabled={
            buttonStatus === 'Loaded!' || buttonStatus === 'Fetching...'
          }
        />
        <Button
          className="h-full"
          label={buttonStatus}
          onClick={() => {
            setGenericFetchReady(true);
          }}
          disabled={
            buttonStatus === 'Loaded!' ||
            buttonStatus === 'Fetching...' ||
            selectedGameType !== 'Generic'
          }
        />
      </div>
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

export default SelectGenericGameCard;
