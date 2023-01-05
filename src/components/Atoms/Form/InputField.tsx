import React from 'react';
import { FieldHookConfig, useField } from 'formik';
import { DISABLE_AUTOFILL } from 'utils/util';

export type InputFieldProps = FieldHookConfig<string> & {
  label?: string;
  type?: 'text' | 'number' | 'password';
};

/**
 * A form input field, meant to be used with Formik
 */
export default function InputField(props: InputFieldProps) {
  const { className, label, type, disabled, ...rest } = props;
  const [field, meta] = useField(props);
  const isError = meta.touched && meta.error && true;
  const errorText = isError ? meta.error : '';

  return (
    <div
      className={`flex flex-col gap-1 ${className} group relative text-base`}
    >
      {label && (
        <label className="absolute -top-6 text-small font-medium text-gray-300">
          {label}:
        </label>
      )}
      <div className="mt-1">
        <input
          className={`input-base w-full ${
            errorText ? 'border-error' : 'border-normal'
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
