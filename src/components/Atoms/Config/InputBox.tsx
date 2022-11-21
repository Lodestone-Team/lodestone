import { faFloppyDisk, faRotateRight } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useCallback, useEffect, useRef, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString, parseintStrict } from 'utils/util';

const onChangeValidateTimeout = 100;
const inputClassName =
  'w-full appearance-none bg-gray-900 py-1.5 px-3 rounded-md outline outline-1 enabled:text-gray-300 tracking-tight leading-snug font-medium focus-visible:ring-4  disabled:text-white/50 disabled:bg-gray-800';
const inputBorderClassName =
  'outline-gray-faded/30 enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

const iconClassName =
  'w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500';

export type InputBoxType = 'text' | 'number';

export default function InputBox({
  label,
  placeholder,
  value: initialValue,
  className,
  onSubmit: onSubmitProp,
  type = 'text',
  min,
  max,
  required,
  maxLength = 1024,
  error: errorProp,
  removeArrows,
  disabled = false,
  id = '',
  showIcons = true,
  validate: validateProp, //throws error if invalid
  onChange: onChangeProp,
}: {
  label?: string;
  placeholder?: string;
  value?: string;
  className?: string;
  type?: InputBoxType;
  min?: number;
  max?: number;
  required?: boolean;
  maxLength?: number;
  error?: string;
  removeArrows?: boolean;
  disabled?: boolean;
  id?: string;
  showIcons?: boolean;
  onSubmit: (arg: string) => Promise<void>;
  validate?: (arg: string) => Promise<void>;
  onChange?: (arg: string) => Promise<void>;
}) {
  const [value, setValue] = useState(initialValue ?? '');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const [touched, setTouched] = useState<boolean>(false);
  const formRef = useRef<HTMLFormElement>(null);

  const validate = useCallback(
    async (value: string, submit: boolean) => {
      if (required && !value && submit) throw new Error('Cannot be empty');
      if (type === 'number') {
        if (!value && submit) {
          throw new Error('Cannot be empty');
        }
        const numValue = parseintStrict(value);
        if (isNaN(numValue)) throw new Error('Must be a number');
        if (min !== undefined && numValue < min)
          throw new Error(`Must be greater than ${min}`);
        if (max !== undefined && numValue > max)
          throw new Error(`Must be less than ${max}`);
      }
      if (validateProp) await validateProp(value);
    },
    [max, min, type, validateProp, required]
  );

  // we want to validate the input after the user stops typing for a while
  useEffect(() => {
    if (!touched) return;
    const timeout = setTimeout(async () => {
      const trimmed = value.trim();
      const error = await catchAsyncToString(validate(trimmed, false));
      setError(error);
      if (error) return;
    }, onChangeValidateTimeout);
    return () => clearTimeout(timeout);
  }, [value, validate, touched]);

  // set touch to false when the value changes
  useEffect(() => {
    setTouched(initialValue !== value);
    if (initialValue === value) setError('');
  }, [initialValue, value]);

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue ?? '');
  }, [initialValue]);

  const onChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setValue(currentText);
    setTouched(true);
    if (onChangeProp) {
      const error = await catchAsyncToString(onChangeProp(currentText));
      if (error) setError(error);
    }
  };

  const onSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    // if (!touched) return;
    const trimmed = value.trim();
    setValue(trimmed);
    setIsLoading(true);
    const validateError = await catchAsyncToString(validate(value, true));
    if (validateError) {
      setError(validateError);
      setIsLoading(false);
      return;
    }
    const submitError = await catchAsyncToString(onSubmitProp(trimmed));
    setError(submitError);
    setIsLoading(false);
  };

  const onReset = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setValue(initialValue ?? '');
    setError('');
    setTouched(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLFormElement>) => {
    if (e.key === 'Escape')
      // escape
      formRef.current?.reset();
  };

  const uiError = errorProp || error;

  let icons = [];

  if (touched) {
    icons.push(
      <FontAwesomeIcon
        icon={faFloppyDisk}
        className={iconClassName}
        onClick={() => formRef.current?.requestSubmit()}
        key="save"
      />
    );
    icons.push(
      <FontAwesomeIcon
        icon={faRotateRight}
        className={iconClassName}
        onClick={() => formRef.current?.reset()}
        key="reset"
      />
    );
  }
  if (isLoading) {
    icons = [
      <BeatLoader
        key="loading"
        size="0.25rem"
        cssOverride={{
          width: '2rem',
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          margin: `0 -0.5rem`,
        }}
        color="#6b7280"
      />,
    ];
  }

  return (
    <div
      className={`flex flex-row items-center justify-between ${className} group relative bg-gray-800 px-4 py-3 text-base`}
    >
      <div className={`flex flex-col`}>
        <label className="text-base font-medium text-gray-300">{label}</label>
        {uiError ? (
          <p className="text-small font-medium tracking-medium text-red">
            {uiError || 'Unknown error'}
          </p>
        ) : (
          <p className="text-small font-medium tracking-medium text-white/50">
            The {label} for the server
          </p>
        )}
      </div>
      <form
        onSubmit={onSubmit}
        onReset={onReset}
        className="relative w-1/2"
        ref={formRef}
        onKeyDown={handleKeyDown}
        id={id}
      >
        <div className={`absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 ${(!removeArrows && type==="number") ? "pl-3 pr-9" : "px-3"}`}>
          <div className="flex flex-row gap-2">{showIcons && icons}</div>
        </div>
        <input
          value={value}
          placeholder={placeholder}
          onChange={onChange}
          maxLength={maxLength}
          className={`${inputClassName} ${
            uiError ? inputErrorBorderClassName : inputBorderClassName
          }
            ${removeArrows && 'noSpin'}`}
          onBlur={() => {
            setValue(value.trim());
          }}
          disabled={disabled}
          type={type}
        />
      </form>
    </div>
  );
}
