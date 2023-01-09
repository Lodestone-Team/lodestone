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
  focus = false,
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
        'text-mediud font-bold tracking-medium hover:bg-gray-700',
        focus && 'outline-fade-700 outline outline-1'
      )}
      onClick={cardOnClick}
    >
      <GameIcon
        game_type={game_type}
        game_flavour={flavour}
        className="h-4 w-4"
      />
      <p className="grow truncate text-medium font-bold tracking-medium text-gray-300">
        {name}
      </p>
      <FontAwesomeIcon
        icon={faCircle}
        className={`select-none text-[8px] ${stateColor}`}
      />
    </div>
  );
}
