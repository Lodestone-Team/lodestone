import { faSquare } from '@fortawesome/free-regular-svg-icons';
import { faCheckSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import clsx from 'clsx';
import { useEffect, useState } from 'react';

/**
 * A checkbox with a label meant to be used as a controlled component and not a form input
 * TODO: improve accessibility and keyboard navigation, maybe use radix-ui to achieve this
 */
export default function Checkbox({
  label,
  checked,
  onChange,
  disabled = false,
  className,
}: {
  label?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
}) {
  const handleClick = (e: React.MouseEvent) => {
    if (disabled) return;
    onChange(!checked);
  };

  return (
    <div
      className={clsx(
        'flex items-center gap-3 text-base font-medium tracking-medium',
        className
      )}
    >
      <div
        className={clsx(
          '-my-2 -mx-2.5 flex h-8 w-8 shrink-0 select-none items-center justify-center overflow-clip rounded-full',
          disabled && 'text-gray-500',
          !disabled && [
            'cursor-pointer hover:bg-gray-faded/30',
            checked && 'text-gray-300 hover:text-gray-300',
            !checked && 'text-gray-400 hover:text-gray-300',
          ]
        )}
        onClick={handleClick}
      >
        <FontAwesomeIcon icon={checked ? faCheckSquare : faSquare} />
      </div>
      {label && (
        <label
          onClick={handleClick}
          className={clsx(
            'truncate',
            disabled && 'text-gray-500',
            !disabled && 'text-gray-300 hover:text-gray-300'
          )}
        >
          {label}
        </label>
      )}
    </div>
  );
}
