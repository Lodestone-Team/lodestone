import { faAngleDown, faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox } from '@headlessui/react';
import { useEffect, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';
import { Switch } from '@headlessui/react';

const inputClassName =
  'w-full bg-gray-900 text-left rounded-md outline outline-1 enabled:text-gray-300 tracking-tight leading-snug font-medium focus-visible:ring-4 disabled:text-white/50 disabled:bg-gray-800 enabled:hover:bg-gray-800';
const inputBorderClassName =
  'outline-gray-faded/30 enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

const iconClassName =
  'w-4 text-gray-faded/30 group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500';

export default function SelectBox({
  label,
  value: initialValue,
  className,
  onChange: onChangeProp,
  error: errorProp,
  disabled = false,
}: {
  label: string;
  value: boolean;
  className?: string;
  error?: string;
  disabled?: boolean;
  onChange: (arg: boolean) => Promise<void>;
}) {
  const [value, setValue] = useState(initialValue);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue);
  }, [initialValue]);

  const onChange = async (newValue: boolean) => {
    setValue(newValue);
    setIsLoading(true);
    const submitError = await catchAsyncToString(onChangeProp(newValue));
    setError(submitError);
    setIsLoading(false);
    if (submitError.length > 0) {
      setValue(initialValue);
    }
  };

  const uiError = errorProp || error;

  const status = isLoading ? (
    <BeatLoader
      key="loading"
      size="0.25rem"
      cssOverride={{
        width: '2rem',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        margin: `0 -0.5rem`,
      }}
      color="#6b7280"
    />
  ) : (
    <p className="text-small font-medium italic text-white/50">
      {disabled ? '' : value ? 'Enabled' : 'Disabled'}
    </p>
  );

  return (
    <div
      className={`flex flex-row items-center justify-between ${className} group relative bg-gray-800 px-4 py-3 text-base`}
    >
      <div className={`flex flex-col`}>
        <label className="text-base font-medium text-gray-300">{label}</label>
        {uiError ? (
          <p className="text-small font-medium tracking-medium text-red">
            {uiError || 'Unknown error'}
          </p>
        ) : (
          <p className="text-small font-medium tracking-medium text-white/50">
            The {label} for the server
          </p>
        )}
      </div>
      <div className="relative flex w-5/12 flex-row items-center justify-end gap-4">
        {status}
        <Switch
          checked={value}
          onChange={onChange}
          className={`${
            disabled
              ? 'bg-gray-faded/30'
              : value
              ? 'bg-green-enabled/50'
              : 'bg-white/50'
          } relative inline-flex h-6 w-11 items-center rounded-full`}
          disabled={disabled || isLoading}
        >
          <span className="sr-only">Enable notifications</span>
          <span
            className={`${value ? 'translate-x-6' : 'translate-x-1'} ${
              disabled || isLoading ? 'bg-gray-faded/40' : 'bg-white'
            } inline-block h-4 w-4 transform rounded-full`}
          />
        </Switch>
      </div>
    </div>
  );
}