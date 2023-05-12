import {
  capitalizeFirstLetter,
  errorToString,
  stateToLabelColor,
} from 'utils/util';
import Label from './Atoms/Label';
import { Fragment, useContext, useState } from 'react';
import axios from 'axios';
import { InstanceState } from 'bindings/InstanceState';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useUserAuthorized } from 'data/UserInfo';
import GameIcon from './Atoms/GameIcon';
import clsx from 'clsx';
import { toast } from 'react-toastify';
import useAnalyticsEventTracker from 'utils/hooks';
import {
  faArrowRotateBackward,
  faPlug,
  faPowerOff,
} from '@fortawesome/free-solid-svg-icons';
import IconButton from './Atoms/IconButton';
import ClipboardTextfield from './ClipboardTextfield';
import { useGlobalSettings } from 'data/GlobalSettings';
import { LodestoneContext } from 'data/LodestoneContext';
import Tooltip from 'rc-tooltip';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Menu, Transition } from '@headlessui/react';
import Avatar from 'boring-avatars';
import Button from './Atoms/Button';

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

  const stateToMenuInfo = () => {
    switch (state) {
      case 'Starting':
        return [{ label: 'Kill', icon: faPlug, onClick: buttonOnClick }];
      case 'Stopping':
        return [{ label: 'Kill', icon: faPlug, onClick: buttonOnClick }];
      case 'Stopped':
        return [{ label: 'Start', icon: faPowerOff, onClick: buttonOnClick }];
      case 'Running':
        return [
          { label: 'Stop', icon: faPowerOff, onClick: buttonOnClick },
          {
            label: 'Restart',
            icon: faArrowRotateBackward,
            onClick: restartButtonOnClick,
          },
        ];
      case 'Error':
        return [
          {
            label: 'Restart',
            icon: faArrowRotateBackward,
            onClick: restartButtonOnClick,
          },
        ];
      default:
        return [];
    }
  };

  const restartButtonOnClick = () => {
    if (loading) return;
    if (disabled) return;

    setLoading(true);

    gaEventTracker('Restarted Instance');
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

    if (state === 'Stopped') {
      gaEventTracker('Started Instance');
    }

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
        <GameIcon game_type={game_type} className="h-4 w-4" />
        <p className="grow truncate">{name}</p>
        <Tooltip
          showArrow={false}
          overlay={<span>Power</span>}
          placement="top"
          trigger={['hover']}
          mouseEnterDelay={0.2}
        >
          <Menu as="div" className="relative">
            <Menu.Button as={IconButton} icon={faPowerOff} />
            <Transition
              as={Fragment}
              enter="transition ease-out duration-200"
              enterFrom="opacity-0 -translate-y-1"
              enterTo="opacity-100 translate-y-0"
              leave="transition ease-in duration-150"
              leaveFrom="opacity-100 translate-y-0"
              leaveTo="opacity-0 -translate-y-1"
            >
              <Menu.Items className="absolute right-0 z-10 mt-1.5 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none">
                <div className="py-2 px-1.5">
                  {stateToMenuInfo().map((menuInfo, i) => (
                    <Menu.Item key={i}>
                      {({ disabled }) => (
                        <Button
                          className="w-full gap-8 whitespace-nowrap"
                          label={menuInfo.label}
                          iconRight={menuInfo.icon}
                          onClick={menuInfo.onClick}
                          align="between"
                          disabled={disabled}
                          variant="text"
                        />
                      )}
                    </Menu.Item>
                  ))}
                </div>
              </Menu.Items>
            </Transition>
          </Menu>
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
