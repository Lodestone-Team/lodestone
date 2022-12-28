import React, { useEffect, useState } from 'react';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { Combobox } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import { faSort, faTrashAlt } from '@fortawesome/free-solid-svg-icons';

export type ComboFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
  allowCustom?: boolean;
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
    ...rest
  } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [query, setQuery] = useState('');
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

  const filteredOptions =
    query === ''
      ? options
      : options.filter((option) => {
          return option.toLowerCase().includes(query.toLowerCase());
        });

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
      className={`w-4 text-gray-faded/30 ${
        disabledVisual || 'group-hover:cursor-pointer group-hover:text-gray-500'
      }`}
    />
  );

  const itemActionIcon = loadingVisual ? (
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
        'w-4 text-gray-faded/30 cursor-pointer hover:text-gray-500'
      }
    />
  );

  function itemAction() {
    // pass
  }

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
          <Combobox.Input
            className={`ui-open:bg-gray-700 ui-not-open:bg-gray-900 input-base group min-h-[1em] w-full py-1.5 px-3 ${
              errorText ? 'border-error' : 'border-normal'
            } ${selectedValue ? 'text-gray-300' : 'text-gray-500'}`}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={placeholder}
          />
          <Combobox.Button className="group absolute inset-y-0 right-0 flex items-center pr-1.5">
            {icon}
          </Combobox.Button>
          <Combobox.Options
            className={`input-base border-normal absolute z-50 mt-2 max-h-60 w-full overflow-auto py-3 p-0 shadow-md rounded-md`}
          >
            {allowCustom && query.length > 0 && (
                <Combobox.Option
                  value={query}
                  className="border border-gray-400/30 relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300 ui-selected:font-medium ui-not-selected:font-normal ui-selected:ui-active:bg-gray-600 ui-selected:ui-not-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800 ui-not-selected:ui-not-active:bg-gray-900"
                >
                  {({ active }) => (
                    <div className="flex flex-row justify-between">
                      <span className="block truncate pr-1">
                        Add &#34;{query}&#34;
                      </span>
                      <div onClick={itemAction} className="right-3 absolute">{active && itemActionIcon}</div>
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
                  className="border border-gray-400/30 relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300 ui-selected:font-medium ui-not-selected:font-normal ui-selected:ui-active:bg-gray-600 ui-selected:ui-not-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800 ui-not-selected:ui-not-active:bg-gray-900"
                >
                  {({ active }) => (
                    <div className="flex flex-row justify-between">
                      <span className="block truncate pr-1">
                        {option}
                      </span>
                      <div onClick={itemAction} className="right-3 absolute">{active && itemActionIcon}</div>
                    </div>
                  )}
                </Combobox.Option>
              ))
            )}
          </Combobox.Options>
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
