import React from 'react';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';

export type InputFieldProps = FieldHookConfig<string> & {
  label?: string;
  type?: 'text' | 'number' | 'password';
};

export default function InputField(props: InputFieldProps) {
  const { className, label, type, ...rest } = props;
  const [field, meta] = useField(props);
  const isError = at(meta, 'touched', 'error').every((v) => v);
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
          autoComplete="off-doesn't-work-but-apparently-this-does"
          placeholder={props.placeholder}
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
