import { Switch } from '@headlessui/react';
import clsx from 'clsx';

export const Toggle = ({
  value,
  onChange,
  disabled = false,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
}: {
  value: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  'aria-label'?: string;
  /** the id of the element that names the switch, e.g. a settings row label */
  'aria-labelledby'?: string;
}) => {
  return (
    <Switch
      checked={value}
      onChange={onChange}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      className={clsx(
        'relative inline-flex h-6 w-11 items-center rounded-full outline-0 transition duration-75 enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        {
          'bg-gray-faded/30': disabled,
          'bg-green-400': value && !disabled,
          'bg-white/50': !value && !disabled,
        }
      )}
      disabled={disabled}
    >
      <span
        className={clsx(
          'inline-block h-4 w-4 rounded-full transition-transform duration-75',
          {
            'translate-x-6': value,
            'translate-x-1': !value,
            'bg-gray-faded/40': disabled,
            'bg-on-accent': !disabled,
          }
        )}
      />
    </Switch>
  );
};
