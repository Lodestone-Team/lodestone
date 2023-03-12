import React, { useEffect, useState } from 'react';
import { FieldHookConfig, useField } from 'formik';
import { RadioGroup } from '@headlessui/react';
import { Toggle } from 'components/Atoms/Toggle';
import Label from 'components/Atoms/Label';

export type RadioFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
  description?: string;
  optional?: boolean;
};

/**
 * A radio button field, meant to be used with Formik.
 */
export default function FormRadioField(props: RadioFieldProps) {
  const {
    label,
    className,
    disabled,
    options,
    loading,
    description,
    optional,
    ...rest
  } = props;
  const [field, meta] = useField(props);
  const { value } = field;
  if (value === undefined) {
    field.onChange({
      target: {
        name: field.name,
        value: false,
      },
    });
  }
  const selectedValue = value === undefined ? false : value; //for initial render for default value
  const [toggleValue, setToggleValue] = useState(selectedValue as boolean);
  const isError = meta.touched && meta.error && true;
  const errorText = isError ? meta.error : '';
  const disabledVisual = disabled || loading;
  const loadingVisual = loading && !disabled;
  // reset the field value if the options change
  useEffect(() => {
    if (selectedValue && !options.includes(selectedValue.toString())) {
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
    <>
      <div
        className={`flex flex-row items-center justify-between ${className} group relative gap-4 bg-gray-800 px-4 py-3 text-h3`}
      >
        <div className={`flex min-w-0 grow flex-col`}>
          {label && (
            <label className="text-medium font-medium text-gray-300">
              {label}
              {optional && (
                <span className="ml-2">
                  <Label size="small" color="green" className="py-[0.2rem]">
                    Optional
                  </Label>
                </span>
              )}
            </label>
          )}
          {errorText ? (
            <div className="text-medium font-medium tracking-medium text-red">
              {errorText || 'Unknown error'}
            </div>
          ) : (
            <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
              {description}
            </div>
          )}
        </div>
        <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
          <Toggle
            value={toggleValue}
            onChange={(newValue: boolean) => {
              // need to generate a fake React.ChangeEvent
              console.log(newValue);
              const event = {
                target: {
                  name: field.name,
                  value: newValue,
                },
              };
              console.log(event);
              field.onChange(event);
              setToggleValue(newValue);
            }}
            disabled={disabledVisual}
          />
        </div>
        {/* <div className="relative mt-1">
          <RadioGroup
            value={selectedValue ? selectedValue.toString() : ''}
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
                className={`input-background input-outlines input-text-style disabled w-full overflow-clip rounded-none p-0 first:rounded-l-md last:rounded-r-md ui-disabled:text-white/50 ui-not-disabled:text-gray-300 ${
                  isError
                    ? 'input-border-error ui-not-disabled:focus-visible:ring-red-faded/30'
                    : 'input-border-normal ui-not-disabled:focus-visible:ring-blue-faded/50'
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
                  className={`input-background input-outlines input-text-style w-full overflow-clip rounded-none p-0 first:rounded-l-md last:rounded-r-md ui-disabled:text-white/50 ui-not-disabled:cursor-pointer ui-not-disabled:text-gray-300
              ${
                isError
                  ? 'input-border-error ui-not-disabled:focus-visible:ring-red-faded/30'
                  : 'input-border-normal ui-not-disabled:focus-visible:ring-blue-faded/50'
              }`}
                >
                  {({ checked }) => {
                    return (
                      <span
                        className={`block h-full w-full select-none py-1.5 px-3 text-center ${
                          disabledVisual
                            ? checked || selectedValue.toString() == option
                              ? 'bg-blue-faded/30 text-white/50'
                              : 'bg-gray-800 text-white/50'
                            : checked || selectedValue.toString() == option
                            ? 'bg-[#2B4554] text-gray-300'
                            : 'text-white/75'
                        }`}
                      >
                        {option}
                      </span>
                    );
                  }}
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

      <div
        className={`flex flex-row items-center justify-between ${className} group relative gap-4 bg-gray-800 px-4 py-3 text-h3`}
      >
        <div className={`flex min-w-0 grow flex-col`}>
          <label className="text-medium font-medium tracking-medium text-gray-300">
            {label}
          </label>
          {errorText ? (
            <div className="text-small font-medium tracking-medium text-red">
              {errorText || 'Unknown error'}
            </div>
          ) : (
            <div className="overflow-hidden text-ellipsis text-medium font-medium tracking-medium text-white/50">
              {description}
            </div>
          )}
        </div>
        <div className="relative flex w-5/12 shrink-0 flex-row items-center justify-end gap-4">
          {status}
          <Toggle
            value={value}
            onChange={onChange}
            disabled={disabled || isLoading}
          />
        </div> */}
      </div>
    </>
  );
}
