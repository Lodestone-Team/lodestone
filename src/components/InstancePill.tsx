import { stateToColor } from 'utils/util';
import { useState } from 'react';
import { InstanceInfo } from 'bindings/InstanceInfo';
import GameIcon from './Atoms/GameIcon';
import clsx from 'clsx';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faCircle } from '@fortawesome/free-solid-svg-icons';

interface InstancePillProps extends InstanceInfo {
  focus?: boolean;
  onClick?: () => void;
}

export default function InstancePill({
  name,
  game_type,
  state,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  port,
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onClick: cardOnClick,
  flavour,
}: InstancePillProps) {
  const stateColor = stateToColor[state];

  return (
    <div
      className={clsx(
        'flex flex-row items-center gap-x-1.5',
        'cursor-pointer rounded-md py-1 px-2',
        'text-medium font-bold leading-5 tracking-medium text-white/50 ui-checked:text-gray-300',
        'ui-checked:bg-gray-800 ui-checked:outline ui-checked:outline-1 ui-checked:outline-fade-700 ui-not-checked:hover:bg-gray-800'
      )}
      onClick={cardOnClick}
    >
      <GameIcon
        game_type={game_type}
        game_flavour={flavour}
        className="h-4 w-4"
      />
      <p className="grow truncate">{name}</p>
      <FontAwesomeIcon
        icon={faCircle}
        className={`select-none text-[8px] ${stateColor}`}
      />
    </div>
  );
}
