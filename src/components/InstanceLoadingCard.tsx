import Button from './Atoms/Button';
import Label from './Atoms/Label';
import GameIcon from './Atoms/GameIcon';
import LoadingStatusIcon from './Atoms/LoadingStatusIcon';
import { EventLevel } from 'bindings/EventLevel';
import { OngoingState } from 'data/NotificationContext';
import ProgressBar from './Atoms/ProgressBar';

const NotificationLevelToBgColorClass = (
  level: EventLevel,
  state?: OngoingState
) => {
  switch (level) {
    case 'Warning':
      return 'bg-ochre';
    case 'Error':
      return 'bg-red';
    default:
      switch (state) {
        case 'done':
          return 'bg-green';
        case 'error':
          return 'bg-red';
        case 'ongoing':
        default:
          return 'bg-blue-accent/50';
      }
  }
};

export default function InstanceLoadingCard({
  uuid,
  name,
  port,
  flavour,
  game_type,
  focus = false,
  progress_percent = 0,
  progress_title = '',
  level,
  state,
}: {
  level: EventLevel;
  state: OngoingState;
  uuid: string;
  name: string;
  port: number;
  flavour: string;
  game_type: string;
  focus?: boolean;
  progress_percent?: number;
  progress_title: string;
}) {
  const stateColor = 'gray';
  const borderClass = 'outline-gray-300 ring-gray-500';
  const actionMessage = 'Start';

  console.log(progress_percent);

  return (
    <div
      className={`group relative flex w-fit select-none flex-col items-stretch gap-y-4 rounded-xl border border-gray-faded/30 bg-gray-800 py-4 px-6 text-base font-semibold tracking-tight ${
        focus ? `bg-gray-900 outline outline-2 ring-[6px] ${borderClass}` : ''
      }`}
    >
      <div className="absolute top-0 left-0 z-10 flex h-full w-full flex-col overflow-clip rounded-xl bg-[#000]/70 backdrop-blur-[1px]">
        {' '}
        <div className="flex w-full grow flex-row items-center justify-center gap-2 text-large">
          <LoadingStatusIcon
            state={state}
            level={level}
            bright={true}
            className="h-5"
          />
          <p className="font-bold tracking-medium">{progress_title}</p>
        </div>
        <ProgressBar
          heightClass="h-1.5"
          progress_percent={progress_percent}
          colorClass={NotificationLevelToBgColorClass(level, state)}
        />
      </div>
      <div className="flex min-w-0 grow flex-col">
        <h1 className="truncate text-gray-300">{name}</h1>
        <div className="flex flex-row items-center gap-x-2">
          <h1 className={`text-${stateColor} -mx-1 truncate px-1`}>00/20</h1>
          <Label size="small" color={stateColor}>
            Stopped
          </Label>
        </div>
      </div>

      <div className="flex flex-row items-center justify-between">
        <Button
          label={actionMessage}
          className="w-20 truncate"
          disabled={true}
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
