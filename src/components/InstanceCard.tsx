import { InstanceState, InstanceStatus } from 'data/InstanceList';
import { capitalizeFirstLetter } from 'utils/util';
import Button from './Button';
import Label, { LabelColor } from './Label';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faClone } from '@fortawesome/free-solid-svg-icons';

// a map from InstanceStatus to string names
// instancestatus is a union type
const statusToColorMap: { [key in InstanceStatus]: LabelColor } = {
  stopped: 'gray',
  running: 'green',
  starting: 'ochre',
  stopping: 'ochre',
  crashed: 'red',
  error: 'red',
};

// for the css style of the double border when focused
const statusToBorderMap: { [key in InstanceStatus]: string } = {
  stopped: 'border-gray-300 ring-gray-500',
  running: 'border-green ring-green-faded/25',
  starting: 'border-ochre ring-ochre-faded/25',
  stopping: 'border-ochre ring-ochre-faded/25',
  crashed: 'border-red ring-red-faded/25',
  error: 'border-red ring-red-faded/25',
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
  onClick: cardOnClick,
}: InstanceCardProps) {
  const buttonOnClick = () => {
    // TODO
  };

  const statusColor = statusToColorMap[status];
  const playerCountColor = status == 'running' ? 'green' : 'gray-500';
  const borderClass = statusToBorderMap[status];

  return (
    <div
      className={`flex flex-col p-3 font-bold tracking-tight bg-gray-800 rounded-xl gap-y-3 w-fit ${
        focus ? `border-2 ring-4 ${borderClass}` : 'm-0.5'
      }`}
    >
      <div className="flex flex-row items-center">
        <div className="flex flex-col min-w-0 grow">
          <div className="flex flex-row gap-x-2">
            <h1 className="text-gray-300 truncate">{name}</h1>
            <Label
              size="small"
              color={statusColor}
              label={capitalizeFirstLetter(status)}
            />
          </div>
          <h1 className={`text-${playerCountColor}`}>
            {playerCount}/{maxPlayerCount} Players
          </h1>
          <div className="flex flex-row gap-x-2 text-small">
            <h1 className="text-gray-300 ">
              {ip}:{port}
              <FontAwesomeIcon className="ml-1 text-gray-500" icon={faClone} />
            </h1>
          </div>
        </div>
        <img
          src="/assets/minecraft-vanilla.png"
          alt={`${type} logo`}
          className="w-8 h-8"
        />
      </div>
      <Button label="Lorem Ipsum" onClick={buttonOnClick} />
    </div>
  );
}
