import {
  capitalizeFirstLetter,
  errorToString,
  stateToLabelColor,
} from 'utils/util';
import Label from './Atoms/Label';
import { useContext, useState } from 'react';
import axios from 'axios';
import { InstanceState } from 'bindings/InstanceState';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useUserAuthorized } from 'data/UserInfo';
import GameIcon from './Atoms/GameIcon';
import clsx from 'clsx';
import { toast } from 'react-toastify';
import useAnalyticsEventTracker from 'utils/hooks';
import { faPowerOff, faRefresh } from '@fortawesome/free-solid-svg-icons';
import IconButton from './Atoms/IconButton';
import ClipboardTextfield from './ClipboardTextfield';
import { useGlobalSettings } from 'data/GlobalSettings';
import { LodestoneContext } from 'data/LodestoneContext';
import Tooltip from 'rc-tooltip';

// // for the css style of the double border when focused
// const stateToBorderMap: { [key in InstanceState]: string[] } = {
//   Starting: ['ui-checked:outline-yellow', 'ui-checked:ring-yellow-faded/25'],
//   Running: ['ui-checked:outline-green', 'ui-checked:ring-green-faded/25'],
//   Stopping: ['ui-checked:outline-yellow', 'ui-checked:ring-yellow-faded/25'],
//   Stopped: ['ui-checked:outline-gray-300', 'ui-checked:ring-gray-500'],
//   Error: ['ui-checked:outline-red', 'ui-checked:ring-red-faded/25'],
//   // Loading: 'outline-gray-300 ring-gray-500',
// };

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

  const { core } = useContext(LodestoneContext);
  const { address } = core;
  const { data: globalSettings } = useGlobalSettings();
  const domain = (globalSettings?.domain ?? address) || 'localhost';

  const canViewInstance = useUserAuthorized('can_view_instance', uuid);
  const canStartInstance = useUserAuthorized('can_start_instance', uuid);
  const canStopInstance = useUserAuthorized('can_stop_instance', uuid);
  const gaEventTracker = useAnalyticsEventTracker('Instance Card');
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

  const restartButtonOnClick = () => {
    if (loading) return;
    if (disabled) return;

    setLoading(true);

    gaEventTracker('Change Instance State', 'Restart');
    axios
      .put(`/instance/${uuid}/restart`)
      .then((response) => {
        console.log(response.data);
        response.data;
      })
      .catch((error) => {
        console.log(error);
        toast.error(errorToString(error));
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const buttonOnClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();

    if (loading) return;
    if (disabled) return;

    setLoading(true);

    gaEventTracker('Change Instance State', stateToActionMessageMap[state]);

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
  const actionMessage = stateToActionMessageMap[state];

  return (
    <div className="my-2 border-b-2 border-gray-faded/30 pb-2">
      <div
        className={clsx(
          'flex flex-row items-center gap-x-1.5',
          'rounded-md',
          'text-medium font-bold leading-5 tracking-medium text-gray-300',
          'ui-checked:bg-gray-800 ui-checked:outline ui-checked:outline-1 ui-checked:outline-fade-700 ui-not-checked:hover:bg-gray-800'
        )}
      >
        <GameIcon
          game_type={game_type}
          game_flavour={flavour}
          className="h-4 w-4"
        />
        <p className="grow truncate">{name}</p>
        <IconButton
          icon={faRefresh}
          onClick={() => state === 'Running' && restartButtonOnClick()}
        />
        <Tooltip
          showArrow={false}
          overlay={<span>{actionMessage}</span>}
          placement="top"
          trigger={['hover']}
          mouseEnterDelay={0.2}
        >
          <IconButton icon={faPowerOff} onClick={buttonOnClick} />
        </Tooltip>
      </div>
      <div
        className={clsx(
          'flex flex-row items-center gap-x-1.5',
          'mt-3 rounded-md',
          'text-medium font-bold leading-5 tracking-medium text-gray-300'
        )}
      >
        <ClipboardTextfield
          className="grow rounded-sm bg-gray-800 p-1 text-small font-medium leading-4 tracking-medium text-gray-300 outline outline-1 outline-gray-faded/30 hover:bg-gray-700 hover:outline-white/50"
          textToCopy={`${domain}:${port}`}
          text="Copy IP"
        />
        <Label
          size="small"
          className="flex grow justify-center"
          color={stateColor}
        >
          {player_count}/{max_player_count} Online
        </Label>
      </div>
    </div>
  );
}
