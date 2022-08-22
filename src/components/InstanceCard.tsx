import { InstanceState, InstanceStatus, updateStatus } from 'data/InstanceList';
import { capitalizeFirstLetter, statusToLabelColor } from 'utils/util';
import Button from './Button';
import Label, { LabelColor } from './Label';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faClone } from '@fortawesome/free-solid-svg-icons';
import { useAppDispatch, useAppSelector } from 'utils/hooks';
import { selectClientInfo } from 'data/ClientInfo';
import { response } from 'msw';
import ClipboardTextField from './ClipboardTextField';

// for the css style of the double border when focused
const statusToBorderMap: { [key in InstanceStatus]: string } = {
  stopped: 'outline-gray-300 ring-gray-500',
  running: 'outline-green ring-green-faded/25',
  starting: 'outline-ochre ring-ochre-faded/25',
  stopping: 'outline-ochre ring-ochre-faded/25',
  crashed: 'outline-red ring-red-faded/25',
  error: 'outline-red ring-red-faded/25',
  loading: 'outline-gray-300 ring-gray-500',
};

const statusToActionMessageMap: { [key in InstanceStatus]: string } = {
  stopped: 'Start',
  running: 'Stop',
  starting: 'Kill',
  stopping: 'Kill',
  crashed: 'Restart',
  error: 'Restart',
  loading: '...',
};

const statusToApiEndpointMap: { [key in InstanceStatus]: string } = {
  stopped: '/start',
  running: '/stop',
  starting: '/kill',
  stopping: '/kill',
  crashed: '/restart',
  error: '/restart',
  loading: '',
};

interface InstanceCardProps extends InstanceState {
  focus?: boolean;
  onClick?: () => void;
}

export default function InstanceCard({
  id,
  name,
  type,
  status,
  playerCount,
  maxPlayerCount,
  ip,
  port,
  focus = false,
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onClick: cardOnClick,
}: InstanceCardProps) {
  const clientInfo = useAppSelector(selectClientInfo);
  const dispatch = useAppDispatch();

  const buttonOnClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    // We only set status to loading
    // Websocket will update the status to the actual status
    if (status === 'loading') return;
    dispatch(updateStatus({ id, status: 'loading' }));
    fetch(`${clientInfo.apiUrl}${statusToApiEndpointMap[status]}/${id}`, {
      method: 'POST',
    })
      .then((response) => {
        // TODO: send notification
      })
      .catch((error) => {
        // dispatch(updateStatus({id, status: 'error'}));
        // TODO: send notification
      });
  };

  const statusColor = statusToLabelColor[status];
  const playerCountColor = status == 'running' ? 'green' : 'gray-500';
  const borderClass = statusToBorderMap[status];
  const actionMessage = statusToActionMessageMap[status];

  return (
    <div
      className={`flex flex-col p-3 font-bold tracking-tight bg-gray-800 rounded-xl gap-y-3 w-fit ${
        focus ? `outline outline-2 ring-4 ${borderClass}` : ''
      }`}
      onClick={cardOnClick}
    >
      <div className="flex flex-row items-center">
        <div className="flex flex-col min-w-0 grow">
          <div className="flex flex-row gap-x-2">
            <h1 className="text-gray-300 truncate">{name}</h1>
            <Label size="small" color={statusColor}>
              {capitalizeFirstLetter(status)}
            </Label>
          </div>
          <h1 className={`text-${playerCountColor} truncate`}>
            {playerCount}/{maxPlayerCount} Players
          </h1>
          <ClipboardTextField
            text={`${ip}:${port}`}
            className="text-gray-300 truncate text-small"
          />
        </div>
        <img
          src="/assets/minecraft-vanilla.png"
          alt={`${type} logo`}
          className="w-8 h-8"
        />
      </div>
      <Button
        label={actionMessage}
        onClick={buttonOnClick}
        disabled={status == 'loading'}
      />
    </div>
  );
}
