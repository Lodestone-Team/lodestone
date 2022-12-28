import React, { useEffect } from 'react';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { Listbox } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import { faSort, faTrashAlt } from '@fortawesome/free-solid-svg-icons';

export type SelectFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
};

export default function SelectField(props: SelectFieldProps) {
  const { label, className, disabled, options, placeholder, loading, ...rest } =
    props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const [touched, error] = at(meta, 'touched', 'error');
  const isError = touched && error && true;
  const errorText = isError ? error : '';
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
      icon={faSort}
      className="w-4 text-gray-faded/30 group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500"
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
            className={`ui-open:bg-gray-700 ui-not-open:bg-gray-900 input-base group min-h-[1em] w-full ${
              errorText ? 'border-error' : 'border-normal'
            } ${selectedValue ? 'text-gray-300' : 'text-gray-500'}`}
          >
            <>
              {selectedValue ? selectedValue : placeholder || 'Select...'}
              <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
                <div className="flex flex-row gap-2">{icon}</div>
              </div>
            </>
            <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
              <div className="flex flex-row gap-2">{icon}</div>
            </div>
          </Listbox.Button>

          <Listbox.Options
            className={`input-base border-normal absolute z-50 mt-2 max-h-60 w-full overflow-auto p-0 py-2 shadow-md`}
          >
            {options.map((option) => (
              <Listbox.Option
                key={option}
                value={option}
                className="border border-gray-400/30 relative cursor-default select-none py-2 pl-3 pr-4 text-gray-300 ui-selected:font-medium ui-not-selected:font-normal ui-selected:ui-active:bg-gray-600 ui-selected:ui-not-active:bg-gray-600 ui-not-selected:ui-active:bg-gray-800 ui-not-selected:ui-not-active:bg-gray-900"
              >
                {({ active }) => (
                    <div className="flex flex-row justify-between">
                      <span
                        className="block truncate pr-1"
                      >
                        {option}
                      </span>
                      <div onClick={itemAction} className="right-3 absolute">{active && itemActionIcon}</div>
                    </div>
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
