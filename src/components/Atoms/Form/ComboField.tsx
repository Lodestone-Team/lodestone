import React, { useEffect, useState } from 'react';
import PropTypes from 'prop-types';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { Combobox } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faAngleDown, faCheck } from '@fortawesome/free-solid-svg-icons';

const inputClassName =
  'w-full bg-gray-900 text-left rounded-md outline outline-1 enabled:text-gray-300 tracking-tight leading-snug font-medium focus-visible:ring-4 disabled:text-white/50 disabled:bg-gray-800';
const inputBorderClassName =
  'outline-gray-faded/30 enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

const iconClassName = 'w-4 text-gray-faded/30';

export type ComboFieldProps = FieldHookConfig<string> & {
  label?: string;
  options: string[];
};

export default function ComboField(props: ComboFieldProps) {
  const { label, className, disabled, options, placeholder, ...rest } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [query, setQuery] = useState('');
  const [touched, error] = at(meta, 'touched', 'error');
  const isError = touched && error && true;
  const uiError = isError ? error : '';

  // reset the field value if the options change
  useEffect(() => {
    if (selectedValue && !options.includes(selectedValue)) {
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
          disabled={disabled}
        >
          <Combobox.Input
            className={`group min-h-[1em] py-1.5 px-3 ${inputClassName} ${
              uiError === '' ? inputBorderClassName : inputErrorBorderClassName
            } ${selectedValue ? 'text-gray-300' : 'text-gray-500'}`}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={placeholder}
          />
          <Combobox.Button className="group absolute inset-y-0 right-0 flex items-center pr-1.5">
            <FontAwesomeIcon
              key="icon"
              icon={faAngleDown}
              className={`${iconClassName} ${
                disabled ||
                'group-hover:cursor-pointer group-hover:text-gray-500'
              }`}
            />
          </Combobox.Button>
          <Combobox.Options
            className={`${inputClassName} ${inputBorderClassName} absolute z-50 mt-2 max-h-60 overflow-auto py-1 shadow-md`}
          >
            {filteredOptions.length === 0 && query !== '' ? (
              <div className="relative cursor-default select-none bg-gray-800 py-2 pl-8 pr-4 text-gray-300">
                Nothing found.
              </div>
            ) : (
              filteredOptions.map((option) => (
                <Combobox.Option
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
                </Combobox.Option>
              ))
            )}
          </Combobox.Options>
        </Combobox>
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
