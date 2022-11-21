import React, { useEffect } from 'react';
import PropTypes from 'prop-types';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';
import { RadioGroup } from '@headlessui/react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faAngleDown, faCheck } from '@fortawesome/free-solid-svg-icons';

const iconClassName =
  'w-4 text-gray-faded/30 group-enabled:group-hover:cursor-pointer group-enabled:group-hover:text-gray-500';

export type RadioFieldProps = FieldHookConfig<string> & {
  label?: string;
  options: string[];
};

export default function RadioField(props: RadioFieldProps) {
  const { label, className, disabled, options, ...rest } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
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

  return (
    <div
      className={`flex flex-col gap-1 ${className} group relative text-base`}
    >
      <label className="absolute -top-6 text-small font-medium text-gray-300">
        {label ? `${label}:` : ''}
      </label>
      <div className="relative mt-1">
        <RadioGroup
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
          className={`flex min-h-[1em] flex-row child:grow ${
            selectedValue ? 'text-gray-300' : 'text-gray-500'
          }`}
        >
          {options.map((option) => (
            <RadioGroup.Option
              value={option}
              key={option}
              className={`w-full overflow-clip bg-gray-900 text-left font-medium leading-snug tracking-tight outline outline-1 first:rounded-l-md last:rounded-r-md focus-visible:ring-4 ${
                disabled
                  ? 'bg-gray-800 text-white/50'
                  : 'cursor-pointer text-gray-300 hover:bg-gray-800'
              } ${
                uiError === ''
                  ? `outline-gray-faded/30 ${
                      disabled || 'focus-visible:ring-blue/30'
                    } invalid:outline-red invalid:focus-visible:outline-red`
                  : `outline-red focus-visible:outline-red ${
                      disabled || 'focus-visible:ring-red-faded/30'
                    }`
              }`}
            >
              {({ checked }) => (
                <span
                  className={`block h-full w-full select-none py-1 text-center ${
                    disabled
                      ? checked
                        ? 'bg-green-enabled/40 text-white/50'
                        : 'bg-gray-800 text-white/50'
                      : checked
                      ? 'bg-green-enabled/50 text-white'
                      : 'text-white/75'
                  }`}
                >
                  {option}
                </span>
              )}
            </RadioGroup.Option>
          ))}
        </RadioGroup>
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
