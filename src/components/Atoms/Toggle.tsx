import { Switch } from '@headlessui/react';
import clsx from 'clsx';

export const Toggle = ({
  value,
  onChange,
  disabled = false,
}: {
  value: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}) => {
  return (
    <Switch
      checked={value}
      onChange={onChange}
      className={clsx(
        'relative inline-flex h-6 w-11 items-center rounded-full outline-0 enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50',
        {
          'bg-gray-faded/30': disabled,
          'bg-green-400': value && !disabled,
          'bg-white/50': !value && !disabled,
        }
      )}
      disabled={disabled}
    >
      <span
        className={clsx('inline-block h-4 w-4 transform rounded-full', {
          'translate-x-6': value,
          'translate-x-1': !value,
          'bg-gray-faded/40': disabled,
          'bg-white': !disabled,
        })}
      />
    </Switch>
  );
};
