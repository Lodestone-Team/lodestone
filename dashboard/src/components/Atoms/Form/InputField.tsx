import React from 'react';
import { FieldHookConfig, useField } from 'formik';
import { DISABLE_AUTOFILL } from 'utils/util';
import { faEye, faEyeSlash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

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

  const [typeModified, setTypeModified] = React.useState(type || 'text');
  return (
    <div
      className={`flex flex-col gap-1 ${className} group relative text-medium`}
    >
      {label && (
        <label className="absolute -top-6 text-medium font-medium tracking-medium text-gray-300">
          {label}:
        </label>
      )}
      {type === 'password' && (
        <div
          className={`pointer-events-none absolute right-0 flex h-full flex-row items-center justify-end py-5 px-3`}
        >
          <div className="pointer-events-auto flex flex-row gap-2">
            <FontAwesomeIcon
              icon={typeModified === 'password' ? faEye : faEyeSlash}
              className="w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500"
              onClick={() => {
                typeModified === 'password'
                  ? setTypeModified('text')
                  : setTypeModified('password');
              }}
              key="reveal password"
            />
          </div>
        </div>
      )}
      <div className="mt-1">
        <input
          className={`input-shape input-background input-outlines input-text-style w-full ${
            errorText ? 'input-border-error' : 'input-border-normal'
          }`}
          type={typeModified}
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
