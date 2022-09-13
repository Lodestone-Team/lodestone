import { capitalizeFirstLetter, stateToLabelColor } from 'utils/util';
import Button from './Button';
import Label, { LabelColor } from './Label';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faClone } from '@fortawesome/free-solid-svg-icons';
import { response } from 'msw';
import ClipboardTextfield from './ClipboardTextfield';
import { useContext, useState } from 'react';
import { LodestoneContext } from 'data/LodestoneContext';
import axios from 'axios';
import { InstanceInfo, InstanceState } from 'data/InstanceList';

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
  port,
  focus = false,
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onClick: cardOnClick,
}: InstanceCardProps) {
  const lodestoneContex = useContext(LodestoneContext);
  const [loading, setLoading] = useState(false);

  const buttonOnClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();

    if (loading) return;
    setLoading(true);

    axios
      .post(`/instances${stateToApiEndpointMap[state]}/${uuid}`)
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
  const playerCountColor = state == 'Running' ? 'green' : 'gray-500';
  const borderClass = stateToBorderMap[state];
  const actionMessage = stateToActionMessageMap[state];

  return (
    <div
      className={`select-none hover:cursor-pointer hover:bg-gray-900 flex flex-col p-3 font-bold tracking-tight bg-gray-800 rounded-xl gap-y-3 w-fit ${
        focus ? `outline bg-gray-900 outline-2 ring-[6px] ${borderClass}` : ''
      }`}
      onClick={cardOnClick}
    >
      <div className="flex flex-row items-center">
        <div className="flex flex-col min-w-0 grow">
          <div className="flex flex-row gap-x-2">
            <h1 className="text-gray-300 truncate">{name}</h1>
            <Label size="small" color={stateColor}>
              {capitalizeFirstLetter(state)}
            </Label>
          </div>
          <h1 className={`text-${playerCountColor} truncate`}>
            {player_count}/{max_player_count} Players
          </h1>
          <ClipboardTextfield
            text={`${lodestoneContex.address}:${port}`}
            className="text-gray-300 truncate text-small"
          />
        </div>
        <img
          src="/assets/minecraft-vanilla.png"
          alt={`${game_type} logo`}
          className="w-8 h-8"
        />
      </div>
      <Button
        label={loading ? '...' : actionMessage}
        onClick={buttonOnClick}
        disabled={loading}
        className="truncate"
      />
    </div>
  );
}
