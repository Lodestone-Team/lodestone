import { useEffect, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';
import { Switch } from '@headlessui/react';

export default function ToggleBox({
  label,
  value: initialValue,
  className,
  onChange: onChangeProp,
  error: errorProp,
  disabled = false,
  canRead = true,
  isLoading: isLoadingProp = false,
  description,
  descriptionFunc,
}: {
  label: string;
  value: boolean;
  className?: string;
  error?: string;
  disabled?: boolean;
  canRead?: boolean;
  isLoading?: boolean;
  onChange: (arg: boolean) => Promise<void>;
  description?: React.ReactNode;
  descriptionFunc?: (arg: boolean) => React.ReactNode;
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

  const errorText = errorProp || error;
  disabled = disabled || !canRead || isLoadingProp;
  description = canRead
    ? descriptionFunc?.(initialValue || value) ?? description
    : 'No permission';

  const status =
    isLoading || isLoadingProp ? (
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
      className={`flex flex-row items-center justify-between ${className} group relative gap-4 bg-gray-800 px-4 py-3 text-base`}
    >
      <div className={`flex min-w-0 grow flex-col`}>
        <label className="text-base font-medium text-gray-300">{label}</label>
        {errorText ? (
          <div className="text-small font-medium tracking-medium text-red">
            {errorText || 'Unknown error'}
          </div>
        ) : (
          <div className="overflow-hidden text-ellipsis text-small font-medium tracking-medium text-white/50">
            {description}
          </div>
        )}
      </div>
      <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
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
          } relative inline-flex h-6 w-11 items-center rounded-full outline-0 enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50`}
          disabled={disabled || isLoading}
        >
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
