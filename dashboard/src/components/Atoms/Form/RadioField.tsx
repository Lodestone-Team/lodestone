import React, { useEffect } from 'react';
import { FieldHookConfig, useField } from 'formik';
import { useT } from 'i18n';
import { RadioGroup } from '@headlessui/react';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';

export type RadioFieldProps = FieldHookConfig<string> & {
  label?: string;
  loading?: boolean;
  options: string[];
  /** the text of an option, when it isn't the option (the value) itself */
  optionLabel?: (option: string) => string;
};

/**
 * A radio button field, meant to be used with Formik.
 */
export default function RadioField(props: RadioFieldProps) {
  const { label, className, disabled, options, optionLabel, loading, ...rest } =
    props;
  const t = useT();
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
        {label ? t('common.fieldLabel', { label }) : ''}
      </label>
      <div className="relative mt-1">
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
            // the options are loading: the buttons' shape (after a moment)
            <SkeletonGroup className="w-full">
              <Skeleton className="h-8.5 w-full rounded-md" />
            </SkeletonGroup>
          ) : (
            options.map((option) => (
              <RadioGroup.Option
                value={option}
                key={option}
                className={`input-background input-outlines input-text-style w-full overflow-clip rounded-none p-0 first:rounded-l-md last:rounded-r-md ui-disabled:text-white/50 ui-not-disabled:cursor-pointer ui-not-disabled:text-gray-300 ${
                  isError
                    ? 'input-border-error ui-not-disabled:focus-visible:ring-red-faded/30'
                    : 'input-border-normal ui-not-disabled:focus-visible:ring-blue-faded/50'
                }`}
              >
                {({ checked }) => {
                  return (
                    <span
                      className={`block h-full w-full px-3 py-1.5 text-center select-none ${
                        disabledVisual
                          ? checked || selectedValue.toString() == option
                            ? 'bg-blue-faded/30 text-white/50'
                            : 'bg-gray-800 text-white/50'
                          : checked || selectedValue.toString() == option
                            ? 'bg-[#2B4554] text-gray-300 light:bg-fill-selected'
                            : 'text-white/75'
                      }`}
                    >
                      {optionLabel ? optionLabel(option) : option}
                    </span>
                  );
                }}
              </RadioGroup.Option>
            ))
          )}
        </RadioGroup>
        {errorText && (
          <div
            className={`absolute -bottom-6 text-right font-sans text-small whitespace-nowrap text-red not-italic`}
          >
            {errorText || t('common.unknownError')}
          </div>
        )}
      </div>
    </div>
  );
}
