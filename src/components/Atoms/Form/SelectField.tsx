import React from 'react';
import PropTypes from 'prop-types';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { Listbox } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faAngleDown, faCheck } from '@fortawesome/free-solid-svg-icons';

const inputClassName =
  'w-full bg-gray-700 text-left rounded-md enabled:outline enabled:outline-2 tracking-tight leading-snug font-medium enabled:focus-visible:ring-[6px] disabled:text-gray-500 disabled:bg-gray-800 enabled:hover:bg-gray-800';
const inputBorderClassName =
  'enabled:outline-gray-400 enabled:focus-visible:outline-blue enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

const iconClassName =
  'w-4 text-gray-faded/30 group-hover:cursor-pointer group-hover:text-gray-500';

export type SelectFieldProps = FieldHookConfig<string> & {
  label?: string;
  options: string[];
};

export default function SelectField(props: SelectFieldProps) {
  const { label, className, disabled, options, ...rest } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [touched, error] = at(meta, 'touched', 'error');
  const isError = touched && error && true;
  const uiError = isError ? error : '';

  return (
    <div className={`flex flex-col gap-1 ${className} group relative`}>
      <label className="absolute font-medium text-gray-300 -top-6 text-small">
        {label ? `${label}:` : ''}
      </label>
      <div className="relative mt-1">
        <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
          <div className="flex flex-row gap-2">
            <FontAwesomeIcon
              key="icon"
              icon={faAngleDown}
              className={iconClassName}
            />
          </div>
        </div>
        <Listbox
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
          <Listbox.Button
            className={`min-h-[1em] py-1.5 px-3 ${inputClassName} ${
              uiError === '' ? inputBorderClassName : inputErrorBorderClassName
            } ${selectedValue ? 'text-gray-300' : 'text-gray-500'}`}
          >
            {selectedValue ? selectedValue : 'Select...'}
          </Listbox.Button>
          <Listbox.Options
            className={`${inputClassName} absolute z-50 mt-2 max-h-60 overflow-auto py-1 shadow-md`}
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
                        <FontAwesomeIcon icon={faCheck} className="w-4 h-4" />
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
