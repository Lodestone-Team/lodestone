import { faSort, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox, Transition } from '@headlessui/react';
import clsx from 'clsx';
import { useEffect, useState, Fragment } from 'react';
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
  actionIcon,
  actionIconClick,
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
  actionIcon?: IconDefinition;
  actionIconClick?: () => any;
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
  description = canRead
    ? descriptionFunc?.(initialValue || value) ?? description
    : 'No permission';

  const icon =
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
      <FontAwesomeIcon
        key="icon"
        icon={faSort}
        className={clsx(
          'w-4 text-gray-faded/30',
          'group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500'
        )}
      />
    );

  return (
    <div
      className={clsx(
        'group relative flex flex-row items-center justify-between',
        'gap-4 bg-gray-800 px-4 py-3 text-base',
        className
      )}
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
      <div className="relative w-5/12 shrink-0">
        <Listbox
          value={value}
          onChange={onChange}
          disabled={disabled || isLoading}
        >
          <Listbox.Button
            className={clsx(
              'input-base group min-h-[1em] w-full py-1.5 px-3',
              'enabled:hover:outline-white/30',
              'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-850',
              'enabled:ui-not-open:bg-gray-850 enabled:ui-not-open:hover:bg-gray-700',
              'enabled:ui-not-open:active:bg-gray-850 enabled:ui-not-open:active:outline-white/30',
              errorText ? 'border-error' : 'border-normal'
            )}
          >
            {value}
            <div
              className={clsx(
                'pointer-events-none absolute items-center justify-end',
                'top-0 right-0 flex h-full flex-row py-1.5 px-3'
              )}
            >
              <div className="flex flex-row gap-2">{icon}</div>
            </div>
          </Listbox.Button>

          <Transition
            as={Fragment}
            enter="transition ease-out duration-200"
            enterFrom="opacity-0 -translate-y-1"
            enterTo="opacity-100 translate-y-0"
            leave="transition ease-in duration-150"
            leaveFrom="opacity-100 translate-y-0"
            leaveTo="opacity-0 -translate-y-1"
          >
            <Listbox.Options
              className={clsx(
                'input-base absolute z-50 mt-2 max-h-60 w-full overflow-auto p-0 py-2',
                'bg-gray-850 outline-gray-550 drop-shadow-md'
              )}
            >
              {options.map((option) => (
                <Listbox.Option
                  key={option}
                  value={option}
                  className={clsx(
                    'relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300',
                    'border border-x-0 border-b-0 border-gray-400/30 last:border-b',
                    'ui-selected:font-medium ui-not-selected:font-normal',
                    'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                    'ui-selected:ui-not-active:bg-gray-600 ui-not-selected:ui-not-active:bg-gray-850'
                  )}
                >
                  {({ active }) => (
                    <div className="flex flex-row justify-between">
                      <span className="block truncate pr-1">{option}</span>
                      <div
                        onClick={actionIconClick}
                        className="absolute right-3"
                      >
                        {active && actionIcon && actionIconClick && (
                          <FontAwesomeIcon
                            key="icon"
                            icon={actionIcon}
                            className="w-4 cursor-pointer text-gray-faded/30 hover:text-gray-500"
                          />
                        )}
                      </div>
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
                  <span className={`block truncate font-normal`}>
                    Select...
                  </span>
                </Listbox.Option>
              )}
            </Listbox.Options>
          </Transition>
        </Listbox>
      </div>
    </div>
  );
}
