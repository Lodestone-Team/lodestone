import GameIcon from 'components/Atoms/GameIcon';
import React, {
  useContext,
  useEffect,
  KeyboardEvent,
  useState,
  useMemo,
} from 'react';
import clsx from 'clsx';
import { Game } from 'bindings/Game';
import Button from 'components/Atoms/Button';
import { GameInstanceContext } from 'data/GameInstanceContext';
import { faCircleInfo } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
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
    url,
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

  const [urlErrorText, setUrlErrorText] = useState('');
  const disableLoadButton = useMemo(() => {
    return (
      buttonStatus === 'Loaded!' ||
      buttonStatus === 'Fetching...' ||
      selectedGameType !== 'Generic' ||
      url === ''
    );
  }, [buttonStatus, selectedGameType, url]);

  useEffect(() => {
    setUrlErrorText(errorText);
  }, [errorText]);

  const loadInstanceFunc = () => {
    if (!isValidUrl(url)) {
      setUrlErrorText('Invalid Url');
      return;
    }
    setUrlErrorText('');
    setGenericFetchReady(true);
  };

  const handleKeyPress = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter' && !disableLoadButton) {
      loadInstanceFunc();
    }
  };

  const isValidUrl = (url: string) => {
    try {
      new URL(url);
      return true;
    } catch (_) {
      return false;
    }
  };

  return (
    <button
      type={'button'}
      className={clsx(
        'relative col-span-2 flex flex-col items-start text-left align-top font-sans tracking-medium',
        'h-36 w-full border-solid p-4 outline outline-1',
        'generic-gametype-unselected enabled:hover:generic-gametype-hover rounded-md border-gray-faded/30 text-gray-faded/30 disabled:text-gray-900',
        'outline-gray-faded/30',
        'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        className
      )}
      onClick={onClick}
    >
      <div className="absolute top-2 right-2">
        <button
          type="button"
          onClick={() => {
            window.open(
              'https://github.com/Lodestone-Team/lodestone/wiki/Lodestone-Atom',
              '_blank'
            );
          }}
          className="w-6 rounded-full hover:text-white/50 focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50"
        >
          <FontAwesomeIcon icon={faCircleInfo} />
        </button>
      </div>
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
            urlErrorText && selectedGameType === 'Generic'
              ? 'input-border-error'
              : 'input-border-normal'
          }`}
          onChange={(e) => {
            if (selectedGameType !== 'Generic') setGameType('Generic');
            setUrl(e.target.value);
            setGenericFetchReady(false);
            setUrlValid(false);
          }}
          name={'Url'}
          placeholder={'Enter URL'}
          disabled={
            buttonStatus === 'Loaded!' || buttonStatus === 'Fetching...'
          }
          onKeyDown={handleKeyPress}
        />
        <Button
          type="button"
          className="h-full"
          label={buttonStatus}
          onClick={loadInstanceFunc}
          disabled={disableLoadButton}
        />
      </div>
      {urlErrorText && selectedGameType === 'Generic' && (
        <div
          className={`mt-1 whitespace-nowrap text-right font-sans text-small not-italic text-red
        `}
        >
          {urlErrorText || 'Unknown error'}
        </div>
      )}
    </button>
  );
};

export default SelectGenericGameCard;
