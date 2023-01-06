import {
  capitalizeFirstLetter,
  errorToString,
  stateToLabelColor,
} from 'utils/util';
import Button from './Atoms/Button';
import Label from './Atoms/Label';
import { useState } from 'react';
import axios from 'axios';
import { InstanceState } from 'bindings/InstanceState';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useUserAuthorized } from 'data/UserInfo';
import GameIcon from './Atoms/GameIcon';
import clsx from 'clsx';
import { toast } from 'react-toastify';
import { Small } from './ClipboardTextfield.stories';

// for the css style of the double border when focused
const stateToBorderMap: { [key in InstanceState]: string[] } = {
  Starting: ['ui-checked:outline-yellow', 'ui-checked:ring-yellow-faded/25'],
  Running: ['ui-checked:outline-green', 'ui-checked:ring-green-faded/25'],
  Stopping: ['ui-checked:outline-yellow', 'ui-checked:ring-yellow-faded/25'],
  Stopped: ['ui-checked:outline-gray-300', 'ui-checked:ring-gray-500'],
  Error: ['ui-checked:outline-red', 'ui-checked:ring-red-faded/25'],
  // Loading: 'outline-gray-300 ring-gray-500',
};

const stateToActionMessageMap: { [key in InstanceState]: string } = {
  Starting: 'Kill',
  Running: 'Stop',
  Stopping: 'Kill',
  Stopped: 'Start',
  Error: 'Restart',
  // loading: '...',
};

const stateToApiEndpointMap: { [key in InstanceState]: string } = {
  Running: '/stop',
  Starting: '/kill',
  Stopping: '/kill',
  Stopped: '/start',
  Error: '/start',
  // Loading: '',
};

interface InstanceCardProps extends InstanceInfo {
  focus?: boolean;
  onClick?: () => void;
}

export default function InstanceCard({
  uuid,
  name,
  game_type,
  state,
  player_count,
  max_player_count,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  port,
  focus = false,
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onClick: cardOnClick,
  flavour,
}: InstanceCardProps) {
  const [loading, setLoading] = useState(false);
  const canViewInstance = useUserAuthorized('can_view_instance', uuid);
  const canStartInstance = useUserAuthorized('can_start_instance', uuid);
  const canStopInstance = useUserAuthorized('can_stop_instance', uuid);
  let disabled = !canViewInstance;
  switch (stateToApiEndpointMap[state]) {
    case '/start':
      if (!canStartInstance) disabled = true;
      break;
    case '/stop':
    case '/kill':
      if (!canStopInstance) disabled = true;
      break;
    default:
      break;
  }

  const buttonOnClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();

    if (loading) return;
    if (disabled) return;

    setLoading(true);

    axios
      .put(`/instance/${uuid}${stateToApiEndpointMap[state]}`)
      .then((response) => {
        response.data;
      })
      .catch((error) => {
        toast.error(errorToString(error));
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const stateColor = stateToLabelColor[state];
  const borderClass = stateToBorderMap[state];
  const actionMessage = stateToActionMessageMap[state];

  return (
    <div
      className={clsx(
        'group flex w-fit select-none flex-col items-stretch gap-y-4 rounded-xl border border-gray-faded/30 bg-gray-800 py-4 px-6 text-medium font-medium tracking-tight hover:cursor-pointer hover:bg-gray-900',
        focus && 'bg-gray-900 outline outline-2 ring-[6px]',
        !focus &&
          'ui-checked:bg-gray-900 ui-checked:outline ui-checked:outline-2 ui-checked:ring-[6px]',
        borderClass
      )}
      onClick={cardOnClick}
    >
      <div className="flex min-w-0 grow flex-col">
        <h1 className="truncate text-gray-300">{name}</h1>
        <div className="flex flex-row items-center gap-x-2">
          <h1 className={`text-${stateColor} -mx-1 truncate px-1`}>
            {player_count}/{max_player_count}
          </h1>
          <Label size="small" color={stateColor}>
            {capitalizeFirstLetter(state)}
          </Label>
        </div>
      </div>

      <div className="flex flex-row items-center justify-between">
        <Button
          label={actionMessage}
          onClick={buttonOnClick}
          loading={loading}
          className="w-20 truncate"
          disabled={disabled}
        />
        <GameIcon
          game_type={game_type}
          game_flavour={flavour}
          className="h-8 w-8"
        />
      </div>
    </div>
  );
}
