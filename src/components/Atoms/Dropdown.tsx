import { faAngleDown, faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Listbox } from '@headlessui/react';
import {
  HTMLInputTypeAttribute,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';

const inputClassName =
  'w-full bg-gray-700 text-left rounded-md enabled:outline enabled:outline-2 enabled:text-gray-300 tracking-tight leading-snug font-medium enabled:focus-visible:ring-[6px] disabled:text-gray-500 disabled:bg-gray-800 enabled:hover:bg-gray-800';
const inputBorderClassName =
  'enabled:outline-gray-400 enabled:focus-visible:outline-blue enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

const iconClassName =
  'w-4 text-gray-faded/30 group-hover:cursor-pointer group-hover:text-gray-500';

export default function Dropdown({
  label,
  value: initialValue,
  options,
  className,
  onChange: onChangeProp,
  error: errorProp,
  disabled = false,
}: {
  label: string;
  value: string;
  options: string[];
  className?: string;
  error?: string;
  disabled?: boolean;
  onChange: (arg: string) => Promise<void>;
}) {
  const [value, setValue] = useState(initialValue);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue);
  }, [initialValue]);

  const onChange = async (newValue: string) => {
    setValue(newValue);
    setIsLoading(true);
    const submitError = await catchAsyncToString(onChangeProp(newValue));
    setError(submitError);
    setIsLoading(false);
    if(submitError.length > 0) {
      setValue(initialValue);
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
    <div className={`flex flex-col gap-1 ${className} group relative`}>
      <label className="block font-medium text-gray-300 text-small">
        {label}:
      </label>
      <div className="relative mt-1">
        <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
          <div className="flex flex-row gap-2">{icon}</div>
        </div>
        <Listbox value={value} onChange={onChange} disabled={disabled || isLoading}>
          <Listbox.Button
            className={`py-1.5 px-3 ${inputClassName} ${
              error === '' ? inputBorderClassName : inputErrorBorderClassName
            }`}
          >
            {value}
          </Listbox.Button>
          <Listbox.Options
            className={`${inputClassName} absolute mt-2 max-h-60 overflow-auto py-1 shadow-md`}
          >
            {options.map((option) => (
              <Listbox.Option
                key={option}
                value={option}
                className={({ active }) => {
                  return `relative cursor-default select-none py-2 pl-8 pr-4 text-gray-300 ${
                    active ? 'bg-gray-800' : 'bg-gray-700'
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
                        <FontAwesomeIcon
                          icon={faCheck}
                          className="w-4 h-4"
                        />
                      </span>
                    )}
                  </>
                )}
              </Listbox.Option>
            ))}
          </Listbox.Options>
        </Listbox>
        {uiError && (
          <div
            className={`absolute -bottom-6 whitespace-nowrap text-right font-sans text-small not-italic text-red
          `}
          >
            {uiError || 'Unknown error'}
          </div>
        )}
      </div>
    </div>
  );
}
