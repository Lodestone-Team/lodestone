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
              const event = {
                target: {
                  name: field.name,
                  value: newValue,
                },
              };
              field.onChange(event);
              setToggleValue(newValue);
            }}
            disabled={disabledVisual}
          />
        </div>
      </div>
    </>
  );
}
