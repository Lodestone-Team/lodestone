import React, { useEffect, useState, Fragment, useMemo } from 'react';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { Combobox, Transition } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import { faSort, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import clsx from 'clsx';

export type ComboFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
  allowCustom?: boolean;
  actionIcon?: IconDefinition;
  actionIconClick?: () => any;
  filterOptions?: (query: string, options: string[]) => string[];
};

const defaultFilterOptions = (query: string, options: string[]) => {
  return query === ''
    ? options
    : options.filter((option) => {
        return option.toLowerCase().includes(query.toLowerCase());
      });
};

export default function ComboField(props: ComboFieldProps) {
  const {
    label,
    className,
    disabled,
    options,
    placeholder,
    loading,
    allowCustom,
    actionIcon,
    actionIconClick,
    filterOptions = defaultFilterOptions,
    ...rest
  } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [query, setQuery] = useState('');
  const buttonRef = React.useRef<HTMLButtonElement>(null);
  const [touched, error] = at(meta, 'touched', 'error');
  const isError = touched && error && true;
  const errorText = isError ? error : '';
  const disabledVisual = disabled || loading;
  const loadingVisual = loading && !disabled;

  // reset the field value if the options change
  useEffect(() => {
    if (selectedValue && !options.includes(selectedValue) && !allowCustom) {
      field.onChange({
        target: {
          name: field.name,
          value: '',
        },
      });
      console.log('resetting field value');
    }
  }, [options, selectedValue]);

  const filteredOptions = useMemo(
    () => filterOptions(query, options),
    [query, options]
  );

  const icon = loadingVisual ? (
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
      className={`flex flex-col gap-1 ${className} group relative text-base`}
    >
      <label className="absolute -top-6 text-small font-medium text-gray-300">
        {label ? `${label}:` : ''}
      </label>
      <div className="relative mt-1">
        <Combobox
          value={selectedValue ? selectedValue : ''}
          name={field.name}
          onChange={(newValue: string) => {
            // need to generate a fake React.ChangeEvent
            const event = {
              target: {
                name: field.name,
                value: newValue,
              },
            };
            field.onChange(event);
          }}
          disabled={disabledVisual}
        >
          {({ open }) => (
            <>
              <Combobox.Input
                className={clsx(
                  'input-base group min-h-[1em] w-full py-1.5 px-3',
                  'enabled:hover:outline-white/30 enabled:ui-not-open:hover:bg-gray-700',
                  'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-850 ',
                  'enabled:ui-not-open:bg-gray-850  enabled:ui-not-open:active:bg-gray-850',
                  'enabled:ui-not-open:active:outline-white/30',
                  errorText ? 'border-error' : 'border-normal',
                  selectedValue ? 'text-gray-300' : 'text-gray-500'
                )}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={placeholder}
                onClick={() => {
                  buttonRef.current?.click();
                }}
              />
              <Combobox.Button
                ref={buttonRef}
                className="group absolute inset-y-0 right-0 flex items-center pr-1.5"
              >
                {icon}
              </Combobox.Button>
              <Transition
                as={Fragment}
                enter="transition ease-out duration-200"
                enterFrom="opacity-0 -translate-y-1"
                enterTo="opacity-100 translate-y-0"
                leave="transition ease-in duration-150"
                leaveFrom="opacity-100 translate-y-0"
                leaveTo="opacity-0 -translate-y-1"
              >
                <Combobox.Options
                  className={clsx(
                    'overflow-y-overlay input-base absolute z-40 mt-2 max-h-60 w-full overflow-auto rounded-md',
                    'bg-gray-850 p-0 py-3 outline-gray-550 drop-shadow-md focus-visible:ring-blue-faded/50'
                  )}
                >
                  {allowCustom && query.length > 0 && (
                    <Combobox.Option
                      value={query}
                      className={clsx(
                        'relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300',
                        'border-t border-gray-faded/30 last:border-b ui-active:border-y ui-active:border-white/50 ui-active:mb-[-1px] ui-active:z-50 ui-active:last:mb-0',
                        'ui-selected:font-medium ui-not-selected:font-normal',
                        'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                        'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
                      )}
                    >
                      {({ active }) => (
                        <div className="flex flex-row justify-between">
                          <span className="block truncate pr-1">
                            Add &#34;{query}&#34;
                          </span>
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
                    </Combobox.Option>
                  )}
                  {filteredOptions.length === 0 && query.length > 0 ? (
                    allowCustom ? null : (
                      <div className="relative cursor-default select-none bg-gray-800 py-2 pl-8 pr-4 text-gray-300">
                        Nothing found.
                      </div>
                    )
                  ) : (
                    filteredOptions.map((option) => (
                      <Combobox.Option
                        key={option}
                        value={option}
                        className={clsx(
                          'relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300',
                          'border-t border-gray-faded/30 last:border-b ui-active:border-y ui-active:border-white/50 ui-active:mb-[-1px] ui-active:z-50 ui-active:last:mb-0',
                          'ui-selected:font-medium ui-not-selected:font-normal',
                          'ui-selected:ui-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800',
                          'ui-selected:ui-not-active:bg-gray-700 ui-not-selected:ui-not-active:bg-gray-850'
                        )}
                      >
                        {({ active }) => (
                          <div className="flex flex-row justify-between">
                            <span className="block truncate pr-1">
                              {option}
                            </span>
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
                      </Combobox.Option>
                    ))
                  )}
                </Combobox.Options>
              </Transition>
            </>
          )}
        </Combobox>
        {errorText && (
          <div
            className={`absolute -bottom-6 whitespace-nowrap text-right font-sans text-small not-italic text-red
          `}
          >
            {errorText || 'Unknown error'}
          </div>
        )}
      </div>
    </div>
  );
}
