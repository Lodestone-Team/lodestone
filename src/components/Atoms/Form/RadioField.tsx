import React, { useEffect } from 'react';
import { FieldHookConfig, useField } from 'formik';
import { RadioGroup } from '@headlessui/react';

export type RadioFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
};

/**
 * A radio button field, meant to be used with Formik.
 */
export default function RadioField(props: RadioFieldProps) {
  const { label, className, disabled, options, loading, ...rest } = props;
  const [field, meta] = useField(props);
  const { value: selectedValue } = field;
  const isError = meta.touched && meta.error && true;
  const errorText = isError ? meta.error : '';
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

  return (
    <div className={`flex flex-col gap-1 ${className} group relative text-h3`}>
      <label className="absolute -top-6 text-medium font-medium tracking-medium text-gray-300">
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
          disabled={disabledVisual}
          className={`flex min-h-[1em] flex-row child:grow ${
            selectedValue ? 'text-gray-300' : 'text-gray-500'
          }`}
        >
          {loadingVisual ? (
            <div
              className={`input-shape input-background input-outlines input-text-style  disabled w-full overflow-clip rounded-none p-0 first:rounded-l-md last:rounded-r-md ${
                isError
                  ? 'input-border-error radio-border-error'
                  : 'input-border-normal radio-border-normal'
              }`}
            >
              <span className="block h-full w-full select-none bg-gray-800 py-1.5 px-3 text-center text-white/50">
                Loading...
              </span>
            </div>
          ) : (
            options.map((option) => (
              <RadioGroup.Option
                value={option}
                key={option}
                className={`input-shape input-background input-outlines input-text-style  w-full overflow-clip rounded-none p-0 first:rounded-l-md last:rounded-r-md ui-not-disabled:cursor-pointer
              ${
                isError
                  ? 'input-border-error radio-border-error'
                  : 'input-border-normal radio-border-normal'
              }`}
              >
                {({ checked }) => (
                  <span
                    className={`block h-full w-full select-none py-1.5 px-3 text-center ${
                      disabledVisual
                        ? checked
                          ? 'bg-blue-faded/30 text-white/50'
                          : 'bg-gray-800 text-white/50'
                        : checked
                        ? 'bg-[#2B4554] text-gray-300'
                        : 'text-white/75'
                    }`}
                  >
                    {option}
                  </span>
                )}
              </RadioGroup.Option>
            ))
          )}
        </RadioGroup>
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
