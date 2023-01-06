import React, { useEffect, Fragment } from 'react';
import { FieldHookConfig, useField } from 'formik';
import { Listbox, Transition } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import BeatLoader from 'react-spinners/BeatLoader';
import { faSort, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import clsx from 'clsx';

export type SelectFieldProps<T extends string | object> = FieldHookConfig<T> & {
  label?: string;
  loading?: boolean;
  options: T[];
  actionIcon?: IconDefinition;
  actionIconClick?: () => void;
  optionLabel?: (option: T) => string;
};

/**
 * A dropdown meant to be used as a form input, with Formik
 */
export default function SelectField<T extends string | object>(
  props: SelectFieldProps<T>
) {
  const {
    label,
    className,
    disabled,
    options,
    placeholder,
    loading,
    actionIcon,
    actionIconClick,
    optionLabel = (option) => {
      let output = '';
      if (typeof option === 'string') {
        output = option;
      } else {
        output = JSON.stringify(option);
      }
      console.log('optionLabel', option, output);
      return output;
    },
    ...rest
  } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const error = meta.error;
  const isError = meta.touched && error && true;
  let errorText = '';
  if (isError && error) {
    if (typeof error === 'string') {
      errorText = error;
    } else if (typeof error === 'object') {
      // for some reason, error is of type "string" when it should be FormikErrors<T>
      // this is a workaround
      errorText = Object.values(error)[0] as string;
    }
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
      icon={faSort}
      className={clsx(
        'w-4 text-gray-faded/30',
        'group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500'
      )}
    />
  );

  return (
    <div
      className={`flex flex-col gap-1 ${className} group relative text-medium`}
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
            className={clsx(
              'input-base group min-h-[1em] w-full py-1.5 px-3',
              'enabled:hover:outline-white/30 enabled:ui-not-open:hover:bg-gray-700',
              'enabled:ui-open:bg-gray-700 enabled:ui-open:active:bg-gray-850 ',
              'enabled:ui-not-open:bg-gray-850  enabled:ui-not-open:active:bg-gray-850',
              'enabled:ui-not-open:active:outline-white/30',
              errorText ? 'border-error' : 'border-normal',
              selectedValue ? 'text-gray-300' : 'text-gray-500'
            )}
          >
            <>
              {selectedValue
                ? optionLabel(selectedValue)
                : placeholder || 'Select...'}
              <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
                <div className="flex flex-row gap-2">{icon}</div>
              </div>
            </>
            <div className="pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3">
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
                'input-base absolute z-40 mt-2 max-h-60 w-full overflow-auto p-0 py-1',
                'bg-gray-850 outline-gray-550 drop-shadow-md focus-visible:ring-blue-faded/50'
              )}
            >
              {options.map((option) => (
                <Listbox.Option
                  key={optionLabel(option)}
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
                      <span className="block truncate pr-1">
                        {optionLabel(option)}
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
                </Listbox.Option>
              ))}
            </Listbox.Options>
          </Transition>
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
