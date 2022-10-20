import React from 'react';
import { at } from 'lodash';
import { FieldHookConfig, useField } from 'formik';

const inputClassName =
  'w-full appearance-none bg-gray-900 py-1.5 px-3 rounded-md outline outline-1 enabled:text-gray-300 tracking-tight leading-snug font-medium focus-visible:ring-4  disabled:text-white/50 disabled:bg-gray-800';
const inputBorderClassName =
  'outline-gray-faded/30 enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

const iconClassName =
  'w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500';

export type InputFieldProps = FieldHookConfig<string> & {
  label?: string;
  type?: 'text' | 'number' | "password";
};

export default function InputField(props: InputFieldProps) {
  const { className, label, type, ...rest } = props;
  const [field, meta] = useField(props);
  const isError = at(meta, 'touched', 'error').every((v) => v);
  const uiError = isError ? meta.error : '';

  return (
    <div className={`flex flex-col gap-1 ${className} group relative text-base`}>
      {label && (
        <label className="absolute font-medium text-gray-300 -top-6 text-small">
          {label}:
        </label>
      )}
      <div className="mt-1">
        <input
          className={`${inputClassName} ${
            uiError ? inputErrorBorderClassName : inputBorderClassName
          }`}
          type={type}
          value={field.value}
          onChange={field.onChange}
          onBlur={field.onBlur}
          name={field.name}
          autoComplete="off-doesn't-work-but-apparently-i-do"
        />
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
