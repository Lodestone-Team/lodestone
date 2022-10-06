import { capitalizeFirstLetter, stateToLabelColor } from 'utils/util';
import Button from './Button';
import Label from './Label';
import {  useState } from 'react';
import axios from 'axios';
import { InstanceState } from 'bindings/InstanceState';
import { InstanceInfo } from 'bindings/InstanceInfo';

// for the css style of the double border when focused
const stateToBorderMap: { [key in InstanceState]: string } = {
  Starting: 'outline-ochre ring-ochre-faded/25',
  Running: 'outline-green ring-green-faded/25',
  Stopping: 'outline-ochre ring-ochre-faded/25',
  Stopped: 'outline-gray-300 ring-gray-500',
  Error: 'outline-red ring-red-faded/25',
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
}: InstanceCardProps) {
  const [loading, setLoading] = useState(false);

  const buttonOnClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();

    if (loading) return;
    setLoading(true);

    axios
      .put(`/instance/${uuid}${stateToApiEndpointMap[state]}`)
      .then((response) => {
        response.data;
      })
      .catch((error) => {
        alert(error);
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
      className={`select-none hover:cursor-pointer hover:bg-gray-900 flex flex-col py-4 px-6 font-semibold tracking-tight bg-gray-800 rounded-xl gap-y-4 w-fit items-stretc group ${
        focus ? `outline bg-gray-900 outline-2 ring-[6px] ${borderClass}` : ''
      }`}
      onClick={cardOnClick}
    >
      <div className="flex flex-col min-w-0 grow">
        <h1 className="text-gray-300 truncate">{name}</h1>
        <div className="flex flex-row items-center gap-x-2">
          <h1 className={`text-${stateColor} truncate px-1 -mx-1`}>
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
        />
        <img
          src="/assets/minecraft-vanilla.png"
          alt={`${game_type} logo`}
          className="w-8 h-8"
        />
      </div>
    </div>
  );
}
