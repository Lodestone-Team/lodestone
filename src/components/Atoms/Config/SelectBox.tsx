import { faSort, faTrashAlt } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox } from '@headlessui/react';
import { useEffect, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';

export default function SelectBox({
  label,
  value: initialValue,
  options,
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
  value?: string;
  options: string[];
  className?: string;
  error?: string;
  disabled?: boolean;
  canRead?: boolean;
  isLoading?: boolean;
  onChange: (arg: string) => Promise<void>;
  description?: React.ReactNode;
  descriptionFunc?: (arg: string) => React.ReactNode;
}) {
  const [value, setValue] = useState(initialValue || 'Select...');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue || 'Select...');
  }, [initialValue]);

  const onChange = async (newValue: string) => {
    setValue(newValue);
    setIsLoading(true);
    const submitError = await catchAsyncToString(onChangeProp(newValue));
    setError(submitError);
    setIsLoading(false);
    if (submitError.length > 0) {
      setValue(initialValue || 'Select...');
    }
  };

  const errorText = errorProp || error;
  disabled = disabled || !canRead || isLoadingProp;
  description = canRead ? descriptionFunc?.(initialValue || value) ?? description : "No permission";

  const icon = (isLoading || isLoadingProp) ? (
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
    <FontAwesomeIcon
      key="icon"
      icon={faSort}
      className={
        'w-4 text-gray-faded/30 group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500'
      }
    />
  );

  const itemActionIcon = (isLoading || isLoadingProp) ? (
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
    />
  ) : (
    <FontAwesomeIcon
      key="icon"
      icon={faTrashAlt}
      className={
        'w-4 text-white/30 group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500'
      }
    />
  );

  return (
    <div
      className={`flex flex-row items-center justify-between ${className} group relative bg-gray-800 px-4 py-3 text-base gap-4`}
    >
      <div className={`flex flex-col min-w-0 grow`}>
        <label className="text-base font-medium text-gray-300">{label}</label>
        {errorText ? (
          <div className="text-small font-medium tracking-medium text-red">
            {errorText || 'Unknown error'}
          </div>
        ) : (
          <div className="text-small font-medium tracking-medium text-white/50 text-ellipsis overflow-hidden">
            {description}
          </div>
        )}
      </div>
      <div className="relative w-5/12 shrink-0">
        <Listbox
          value={value}
          onChange={onChange}
          disabled={disabled || isLoading}
        >
          <Listbox.Button
            className={`input-base group w-full ${
              errorText ? 'border-error' : 'border-normal'
            }`}
          >
            {value}
            <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
              <div className="flex flex-row gap-2">{icon}</div>
            </div>
          </Listbox.Button>
          <Listbox.Options
            className={`input-base border-normal absolute z-50 mt-2 max-h-60 w-full overflow-auto p-0 py-2 shadow-md`}
          >
            {options.map((option ) => (
              <Listbox.Option
                key={option}
                value={option}
                className={({ active, selected }) => {
                  return `border border-gray-400/30 relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300 ${
                    selected ? 'bg-gray-600' : active ? 'bg-gray-800' : 'bg-gray-900'
                  }`;
                }}
              >
                {({ active, selected }) => (
                    <div className="flex flex-row justify-between">
                      <span
                        className={`block truncate ${
                          selected ? 'font-bold' : 'font-normal'
                        }`}
                      >
                        {option}
                      </span>
                      <div className="right-0">{active ? itemActionIcon : ''}</div>
                    </div>
                )}
              </Listbox.Option>
            ))}
            {(initialValue === undefined || initialValue.length === 0) && (
              <Listbox.Option 
                key={'Select...'}
                value={''}
                className={`relative cursor-default select-none bg-gray-700 py-2 pl-8 pr-4 text-gray-400`}
                disabled
              >
                <span className={`block truncate font-normal`}>Select...</span>
              </Listbox.Option>
            )}
          </Listbox.Options>
        </Listbox>
      </div>
    </div>
  );
}
