import { faAngleDown, faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox } from '@headlessui/react';
import { useEffect, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';

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
  options,
  className,
  onChange: onChangeProp,
  error: errorProp,
  disabled = false,
}: {
  label: string;
  value?: string;
  options: string[];
  className?: string;
  error?: string;
  disabled?: boolean;
  onChange: (arg: string) => Promise<void>;
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

  const uiError = errorProp || error;

  const icon = isLoading ? (
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
    <FontAwesomeIcon key="icon" icon={faAngleDown} className={iconClassName} />
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
      <div className="relative w-1/2">
        <Listbox
          value={value}
          onChange={onChange}
          disabled={disabled || isLoading}
        >
          <Listbox.Button
            className={`group py-1.5 px-3 ${inputClassName} ${
              uiError === '' ? inputBorderClassName : inputErrorBorderClassName
            }`}
          >
            {value}
            <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
              <div className="flex flex-row gap-2">{icon}</div>
            </div>
          </Listbox.Button>
          <Listbox.Options
            className={`${inputClassName} ${inputBorderClassName} absolute z-50 mt-2 max-h-60 overflow-auto py-1 shadow-md`}
          >
            {options.map((option) => (
              <Listbox.Option
                key={option}
                value={option}
                className={({ active }) => {
                  return `relative cursor-default select-none py-2 pl-8 pr-4 text-gray-300 ${
                    active ? 'bg-gray-800' : 'bg-gray-900'
                  }`;
                }}
              >
                {({ selected }) => (
                  <>
                    <span
                      className={`block truncate ${
                        selected ? 'font-medium' : 'font-normal'
                      }`}
                    >
                      {option}
                    </span>
                    {selected && (
                      <span className="absolute inset-y-0 left-0 flex items-center pl-2.5 text-green-accent">
                        <FontAwesomeIcon icon={faCheck} className="h-4 w-4" />
                      </span>
                    )}
                  </>
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
