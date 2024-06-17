import { faSort, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox, Transition } from '@headlessui/react';
import clsx from 'clsx';
import { useEffect, useState, Fragment } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';
import { Float } from '@headlessui-float/react';

/**
 * A self controlled dropdown meant to represent a single value of a config
 *
 * It is NOT meant to be used as a form input
 *
 * See SelectField for that
 */
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
  optimistic = true, // if true, the dropdown will change immediately and go into loading state, and will change back if onChange throws an error
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
  actionIconClick?: () => void;
  optimistic?: boolean;
}) {
  const [value, setValue] = useState(initialValue || 'Select...');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue || 'Select...');
  }, [initialValue]);

  const onChange = async (newValue: string) => {
    if (optimistic) setValue(newValue);
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
        'gap-4 bg-gray-800 px-4 py-3 text-medium',
        className
      )}
    >
      <div className={`flex min-w-0 grow flex-col`}>
        <label className="text-medium font-medium tracking-medium text-gray-300">
          {label}
        </label>
        {errorText ? (
          <div className="font-small text-medium tracking-medium text-red">
            {errorText || 'Unknown error'}
          </div>
        ) : (
          <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
            {description}
          </div>
        )}
      </div>
      <div className="relative w-5/12 shrink-0" >
        <Listbox
          value={value}
          onChange={onChange}
          disabled={disabled || isLoading}
        >
          <Float
          as="div"
          className="relative w-full"
          placement="bottom"
          floatingAs={Fragment}
          portal
          adaptiveWidth
          >
          <Listbox.Button
            className={clsx(
              'input-outlines input-text-style group min-h-[1em] w-full rounded py-1.5 px-3',
              'enabled:hover:outline-white/30',
              'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-850',
              'enabled:ui-not-open:bg-gray-850 enabled:ui-not-open:hover:bg-gray-700',
              'enabled:ui-not-open:active:bg-gray-850 enabled:ui-not-open:active:outline-white/30',
              errorText ? 'input-border-error' : 'input-border-normal'
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
            leave="transition ease-in duration-150"
            leaveFrom="opacity-100 translate-y-0"
            leaveTo="opacity-0 -translate-y-1"
          >
              <Listbox.Options
                className={clsx(
                  'input-outlines input-text-style absolute z-40 mt-2 max-h-60 w-full overflow-auto rounded p-0 py-1',
                  'bg-gray-850 outline-gray-550 drop-shadow-md focus-visible:ring-blue-faded/50'
                )}
              >
                {options.map((option) => (
                  <Listbox.Option
                    key={option}
                    value={option}
                    className={clsx(
                      'relative cursor-pointer select-none py-2 pl-3 pr-4 text-gray-300',
                      'border-t border-gray-faded/30 last:border-b ui-active:z-50 ui-active:mb-[-1px] ui-active:border-y ui-active:border-white/50 ui-active:last:mb-0',
                      'ui-selected:font-medium ui-not-selected:font-medium',
                      'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                      'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
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
                    <span className={`block truncate font-medium`}>
                      Select...
                    </span>
                  </Listbox.Option>
                )}
              </Listbox.Options>
          </Transition>
          </Float>
        </Listbox>
      </div>
    </div>
  );
}
