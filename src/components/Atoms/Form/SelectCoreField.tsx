import React, { useEffect } from 'react';
import PropTypes from 'prop-types';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { Listbox } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import { faCaretDown, faCheck } from '@fortawesome/free-solid-svg-icons';
import { CoreConnectionInfo } from 'data/LodestoneContext';
import { isType } from 'variant';

export type SelectCoreFieldProps = FieldHookConfig<CoreConnectionInfo> & {
  label?: string;
  loading?: boolean;
  options: CoreConnectionInfo[];
};

export default function SelectCoreField(props: SelectCoreFieldProps) {
  const { label, className, disabled, options, placeholder, loading, ...rest } =
    props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [touched, error] = at(meta, 'touched', 'error');
  const isError = touched && error && true;
  let errorText = '';
  if (typeof error === 'string') {
    errorText = isError ? error : '';
  } else if (typeof error === 'object') {
    errorText = isError ? error.address : '';
  }
  const disabledVisual = disabled || loading;
  const loadingVisual = loading && !disabled;

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
      icon={faCaretDown}
      className="w-4 text-gray-faded/30 group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500"
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
          disabled={disabledVisual}
        >
          <Listbox.Button
            className={`input-base group min-h-[1em] w-full ${
              errorText ? 'border-error' : 'border-normal'
            } ${selectedValue ? 'text-gray-300' : 'text-gray-500'}`}
          >
            <>
              {selectedValue
                ? `${selectedValue.address}:${selectedValue.port}`
                : placeholder || 'Select...'}
              <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
                <div className="flex flex-row gap-2">{icon}</div>
              </div>
            </>
          </Listbox.Button>
          <Listbox.Options
            className={`input-base border-normal absolute z-50 mt-2 max-h-60 w-full overflow-auto p-0 py-1 shadow-md`}
          >
            {options.map((option) => (
              <Listbox.Option
                key={`${option.address}:${option.port}`}
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
                      {option.address}:{option.port}
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
          </Listbox.Options>
        </Listbox>
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
