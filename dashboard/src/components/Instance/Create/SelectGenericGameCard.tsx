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
import { useT } from 'i18n';
const SelectGenericGameCard = ({
  title,
  game_type,
  className,
  onClick,
  manifestLoading,
  errorText,
}: {
  title: string;
  game_type: Game;
  className?: string;
  onClick?: () => void;
  manifestLoading: boolean;
  errorText: string;
}) => {
  const t = useT();
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
        : 'Load Server'
    : 'Load Server';

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
    setUrlErrorText('');
    setGenericFetchReady(true);
  };

  const handleKeyPress = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter' && !disableLoadButton) {
      loadInstanceFunc();
    }
  };

  return (
    <div
      className={clsx(
        'relative col-span-2 flex flex-row items-start text-left align-top font-sans tracking-medium',
        'w-full border-solid p-4 outline-1 outline-solid',
        'generic-gametype-unselected rounded-md border-gray-faded/30 text-gray-faded/30 enabled:hover:generic-gametype-hover disabled:text-gray-900',
        'outline-gray-faded/30',
        'focus-visible:outline-hidden enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        className
      )}
      onClick={onClick}
    >
      <div className="relative">
        <GameIcon game_type={game_type} className="absolute h-6 w-6" />
        <div
          className="ml-8 text-h3 leading-6 font-mediumbold text-gray-300 hover:cursor-pointer hover:underline"
          onClick={() => {
            window
              .open(
                'https://github.com/Lodestone-Team/lodestone/wiki/Docker-Instance',
                '_blank'
              )
              ?.focus();
          }}
        >
          {title}
        </div>
      </div>

      <div className="shrink-0 pl-2 text-small whitespace-nowrap text-yellow-300">
        {t('create.gameType.new')}
      </div>

      {urlErrorText && selectedGameType === 'Generic' && (
        <div
          className={`mt-1 text-right font-sans text-small whitespace-nowrap text-red not-italic`}
        >
          {urlErrorText || t('common.unknownError')}
        </div>
      )}
    </div>
  );
};

export default SelectGenericGameCard;
