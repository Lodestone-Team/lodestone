import React from 'react';
import { FieldHookConfig, useField } from 'formik';
import { DISABLE_AUTOFILL } from 'utils/util';
import Label from 'components/Atoms/Label';

export type InputFieldProps = FieldHookConfig<string> & {
  label?: string;
  type?: 'text' | 'number' | 'password';
  description?: string;
  optional?: boolean;
};

/**
 * A form input field, meant to be used with Formik
 */
export default function FormInputField(props: InputFieldProps) {
  const { className, label, type, disabled, description, optional, ...rest } =
    props;
  const [field, meta] = useField(props);
  const isError = meta.touched && meta.error && true;
  const errorText = isError ? meta.error : '';
  return (
    <div
      className={`flex flex-row items-center justify-between ${className} group relative gap-4 bg-gray-800 px-4 py-3 text-medium`}
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

      <div className="relative w-5/12 flex-shrink-0">
        <input
          className={`input-shape input-background input-outlines input-text-style w-full ${
            errorText ? 'input-border-error' : 'input-border-normal'
          }`}
          type={type}
          value={field.value}
          onChange={field.onChange}
          onBlur={field.onBlur}
          name={field.name}
          autoComplete={DISABLE_AUTOFILL}
          placeholder={props.placeholder}
          disabled={disabled}
        />
      </div>
    </div>
  );
}
