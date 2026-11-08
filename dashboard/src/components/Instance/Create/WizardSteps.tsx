import { faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { cn } from 'utils/util';

/**
 * The steps of the create dialogs, down a narrow rail: a check for each step
 * that's done, the current step's number filled and its row highlighted, and
 * the steps to come muted. A step that's done can be clicked to go back to it
 * when `onSelect` allows it; steps to come can't be skipped to.
 */
export default function WizardSteps({
  steps,
  current,
  onSelect,
  canSelect = () => true,
}: {
  steps: string[];
  current: number;
  /** go back to step `i` (only offered for steps before `current`) */
  onSelect?: (i: number) => void;
  /** whether a step that's done can be gone back to */
  canSelect?: (i: number) => boolean;
}) {
  return (
    <ol className="flex w-36 shrink-0 flex-col border-r border-gray-700 px-2 pt-8">
      {steps.map((label, i) => {
        const done = i < current;
        const active = i === current;
        const last = i === steps.length - 1;
        const clickable = done && !!onSelect && canSelect(i);
        const marker = (
          <span
            className={cn(
              'z-10 flex size-5 shrink-0 items-center justify-center rounded-full text-caption font-bold',
              done && 'bg-green-300 text-gray-900',
              active && 'bg-blue text-on-accent',
              !done &&
                !active &&
                'border border-gray-faded/30 bg-gray-850 text-gray-500'
            )}
          >
            {done ? (
              <FontAwesomeIcon icon={faCheck} className="size-2.5" />
            ) : (
              i + 1
            )}
          </span>
        );
        const content = (
          <>
            {marker}
            <span className="min-w-0 pt-0.5 leading-4">{label}</span>
          </>
        );
        const rowClass = cn(
          'relative flex w-full flex-row items-start gap-2 rounded-lg p-2 text-left text-small font-mediumbold',
          active ? 'bg-gray-800 text-gray-300' : 'text-gray-500',
          done && 'text-gray-400',
          clickable &&
            'cursor-pointer hover:text-gray-300 focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden'
        );
        return (
          <li key={i} className="relative">
            {/* the line down to the next step's circle */}
            {!last && (
              <span
                aria-hidden
                className={cn(
                  'absolute top-7 -bottom-1 left-4.5 w-px',
                  done ? 'bg-green-300/60' : 'bg-gray-faded/30'
                )}
              />
            )}
            {clickable ? (
              <button
                type="button"
                className={rowClass}
                onClick={() => onSelect(i)}
              >
                {content}
              </button>
            ) : (
              <div
                className={rowClass}
                aria-current={active ? 'step' : undefined}
              >
                {content}
              </div>
            )}
          </li>
        );
      })}
    </ol>
  );
}
