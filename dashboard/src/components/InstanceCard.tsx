import { errorToString, stateToLabelColor } from 'utils/util';
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
  faLink,
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
import { useI18n } from 'i18n';
import ConfirmDialog from './Atoms/ConfirmDialog';
import { MinecraftEulaLink } from './Instance/Create/EulaCheckbox';
import { acceptInstanceEula, getInstanceEula } from 'data/MinecraftEula';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { useInstanceShare } from 'data/InstanceShare';
import { SHARE_SECTION_HASH } from './Instance/InstanceShareSection';

// // for the css style of the double border when focused
// const stateToBorderMap: { [key in InstanceState]: string[] } = {
//   Starting: ['ui-checked:outline-yellow', 'ui-checked:ring-yellow-faded/25'],
//   Running: ['ui-checked:outline-green', 'ui-checked:ring-green-faded/25'],
//   Stopping: ['ui-checked:outline-yellow', 'ui-checked:ring-yellow-faded/25'],
//   Stopped: ['ui-checked:outline-gray-300', 'ui-checked:ring-gray-500'],
//   Error: ['ui-checked:outline-red', 'ui-checked:ring-red-faded/25'],
//   // Loading: 'outline-gray-300 ring-gray-500',
// };

// The power icon says the state, as the clients' server rows do (the
// launcher's `state_color`): green running, yellow starting or stopping, red
// after a crash, grey stopped. Hover keeps the colour.
const stateToPowerIconClass: { [key in InstanceState]: string } = {
  Running: 'text-green-200 enabled:hover:text-green-200',
  Starting: 'text-yellow-300 enabled:hover:text-yellow-300',
  Stopping: 'text-yellow-300 enabled:hover:text-yellow-300',
  Error: 'text-red-200 enabled:hover:text-red-200',
  Stopped: 'text-gray-500 enabled:hover:text-gray-500',
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
  icon_url,
  state,
  player_count,
  max_player_count,
  port,
  focus = false,
  onClick: cardOnClick,
}: InstanceCardProps) {
  const { t, rich } = useI18n();
  const [loading, setLoading] = useState(false);
  // open when a start failed because the Minecraft EULA isn't accepted
  const [eulaDialogOpen, setEulaDialogOpen] = useState(false);

  const { core } = useContext(LodestoneContext);
  const { address } = core;
  const { data: globalSettings } = useGlobalSettings();
  const domain = globalSettings?.domain || address || 'localhost';

  const canViewInstance = useUserAuthorized('can_view_instance', uuid);
  const canStartInstance = useUserAuthorized('can_start_instance', uuid);
  const canStopInstance = useUserAuthorized('can_stop_instance', uuid);
  // Share opens the Overview's Share section, which only Minecraft Java servers
  // have, for users who may manage them, on cores that can share; the others
  // keep Copy IP. (The same query as the section's, so it is asked once.)
  const isJava = game_type.type === 'MinecraftJava';
  const { isError: shareUnavailable } = useInstanceShare(uuid, isJava);
  const canShare = isJava && !shareUnavailable;
  const { setLocation } = useContext(BrowserLocationContext);
  const openShare = () =>
    setLocation((loc) => ({
      ...loc,
      pathname: '/dashboard/overview',
      hash: SHARE_SECTION_HASH,
    }));
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

  // what the signed-in user may do, as the core checks it: a restart stops
  // and starts the server
  const mayPower = (action: 'start' | 'stop' | 'restart' | 'kill') => {
    switch (action) {
      case 'start':
        return canStartInstance;
      case 'stop':
      case 'kill':
        return canStopInstance;
      case 'restart':
        return canStartInstance && canStopInstance;
    }
  };

  const stateToMenuInfo = () => {
    const kill = {
      action: 'kill',
      label: t('server.action.kill'),
      icon: faPlug,
      onClick: powerButtonOnClick,
    } as const;
    const restart = {
      action: 'restart',
      label: t('server.action.restart'),
      icon: faArrowRotateBackward,
      onClick: restartButtonOnClick,
    } as const;
    switch (state) {
      case 'Starting':
        return [kill];
      case 'Stopping':
        return [kill];
      case 'Stopped':
        return [
          {
            action: 'start',
            label: t('server.action.start'),
            icon: faPowerOff,
            onClick: powerButtonOnClick,
          } as const,
        ];
      case 'Running':
        return [
          {
            action: 'stop',
            label: t('server.action.stop'),
            icon: faPowerOff,
            onClick: powerButtonOnClick,
          } as const,
          restart,
        ];
      case 'Error':
        return [restart];
      default:
        return [];
    }
  };

  // A start (or restart) failed. If it's a Minecraft server whose EULA isn't
  // accepted, ask the user to agree to it; otherwise show the error.
  const onStartFailed = async (error: unknown) => {
    if (game_type.type === 'MinecraftJava') {
      try {
        const eula = await getInstanceEula(uuid);
        if (!eula.accepted) {
          setEulaDialogOpen(true);
          return;
        }
      } catch {
        // show the start error below
      }
    }
    toast.error(errorToString(error));
  };

  const acceptEulaAndStart = async () => {
    try {
      await acceptInstanceEula(uuid);
    } catch (error) {
      toast.error(
        t('server.eula.acceptFailed', { error: errorToString(error) })
      );
      return;
    }
    setEulaDialogOpen(false);
    gaEventTracker('Started Instance');
    try {
      await axios.put(`/instance/${uuid}/start`);
    } catch (error) {
      toast.error(errorToString(error));
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
        return onStartFailed(error);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const powerButtonOnClick = async (e: React.MouseEvent<HTMLButtonElement>) => {
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
        if (stateToApiEndpointMap[state] === '/start')
          return onStartFailed(error);
        toast.error(errorToString(error));
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const stateColor = stateToLabelColor[state];
  const menuInfo = stateToMenuInfo().filter((info) => mayPower(info.action));
  // the card's second row: a small outlined button, as Copy IP was
  const chipClass =
    'grow rounded-xs bg-gray-800 p-1 text-small leading-4 font-medium tracking-medium text-gray-300 outline-1 outline-gray-faded/30 outline-solid hover:bg-gray-700 hover:outline-white/50';

  return (
    <div className="my-2 border-b-2 border-gray-faded/30 pb-2">
      <ConfirmDialog
        title={t('server.eula.title')}
        type="info"
        isOpen={eulaDialogOpen}
        onClose={() => setEulaDialogOpen(false)}
        onConfirm={acceptEulaAndStart}
        confirmButtonText={t('server.eula.accept')}
      >
        {rich('server.eula.body', { name, link: <MinecraftEulaLink /> })}
      </ConfirmDialog>
      <div
        className={clsx(
          'flex flex-row items-center gap-x-1.5',
          'rounded-md',
          'text-medium leading-5 font-bold tracking-medium text-gray-300',
          'ui-checked:bg-gray-800 ui-checked:outline-1 ui-checked:outline-fade-700 ui-checked:outline-solid ui-not-checked:hover:bg-gray-800'
        )}
      >
        <GameIcon
          game_type={game_type}
          icon_url={icon_url}
          className="h-4 w-4"
        />
        <p className="grow truncate">{name}</p>
        <Tooltip
          showArrow={false}
          overlay={<span>{t('server.card.power')}</span>}
          placement="top"
          trigger={['hover']}
          mouseEnterDelay={0.2}
        >
          <Menu as="div" className="relative">
            <Menu.Button
              as={IconButton}
              icon={faPowerOff}
              aria-label={`${t('server.card.power')}: ${t(`common.state.${state}`)}`}
              className={stateToPowerIconClass[state]}
              disabled={menuInfo.length === 0}
            />
            {/* opens with a short fade and scale from the button's corner;
                with reduced motion, only the fade */}
            <Transition
              as={Fragment}
              enter="transition ease-out duration-150"
              enterFrom="opacity-0 scale-95 -translate-y-1 motion-reduce:scale-100 motion-reduce:translate-y-0"
              enterTo="opacity-100 scale-100 translate-y-0"
              leave="transition ease-in duration-100"
              leaveFrom="opacity-100 scale-100 translate-y-0"
              leaveTo="opacity-0 scale-95 -translate-y-1 motion-reduce:scale-100 motion-reduce:translate-y-0"
            >
              <Menu.Items className="absolute right-0 z-10 mt-1.5 origin-top-right divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-hidden">
                <div className="px-1.5 py-2">
                  {menuInfo.map((menuInfo, i) => (
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
          'text-medium leading-5 font-bold tracking-medium text-gray-300'
        )}
      >
        {canShare ? (
          <Tooltip
            showArrow={false}
            overlay={<span>{t('server.card.shareHint')}</span>}
            placement="bottom"
            trigger={['hover']}
            mouseEnterDelay={0.2}
          >
            <button
              type="button"
              onClick={openShare}
              className={clsx(
                chipClass,
                'group flex cursor-pointer flex-row items-center justify-center gap-2 whitespace-nowrap select-none',
                'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden'
              )}
            >
              <FontAwesomeIcon
                icon={faLink}
                className="text-gray-faded/25 group-hover:text-gray-500"
              />
              {t('server.card.share')}
            </button>
          </Tooltip>
        ) : (
          <ClipboardTextfield
            className={chipClass}
            textToCopy={`${domain}:${port}`}
            text={t('server.card.copyIp')}
          />
        )}
        <Label
          size="small"
          className="flex grow justify-center whitespace-nowrap"
          color={stateColor}
        >
          {t('server.card.online', {
            players: player_count ?? '',
            max: max_player_count ?? '',
          })}
        </Label>
      </div>
    </div>
  );
}
