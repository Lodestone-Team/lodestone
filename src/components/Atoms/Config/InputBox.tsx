import {
  faFloppyDisk,
  faRotateRight,
  faEye,
  faEyeSlash,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useCallback, useEffect, useRef, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import {
  catchAsyncToString,
  DISABLE_AUTOFILL,
  parseFloatStrict,
  parseintStrict,
} from 'utils/util';

const onChangeValidateTimeout = 100;

export type InputBoxType = 'text' | 'number' | 'password';

/**
 * A self controlled input box meant to represent a single value of a config
 *
 * It is NOT meant to be used as a form input
 *
 * See InputField for that
 */
export default function InputBox({
  label,
  placeholder,
  value: initialValue,
  className,
  onSubmit: onSubmitProp,
  type = 'text',
  isFloat = false,
  min,
  max,
  required,
  maxLength = 1024,
  error: errorProp,
  disabled = false,
  canRead = true,
  isLoading: isLoadingProp = false,
  id = '',
  showIcons = true,
  validate: validateProp, //throws error if invalid
  onChange: onChangeProp,
  description,
  descriptionFunc,
}: {
  label?: string;
  placeholder?: string;
  value?: string;
  className?: string;
  type?: InputBoxType;
  isFloat?: boolean;
  min?: number | null;
  max?: number | null;
  required?: boolean;
  maxLength?: number;
  error?: string;
  disabled?: boolean;
  canRead?: boolean;
  isLoading?: boolean;
  id?: string;
  showIcons?: boolean;
  onSubmit: (arg: string) => Promise<void>;
  validate?: (arg: string) => Promise<void>;
  onChange?: (arg: string) => Promise<void>;
  description?: React.ReactNode;
  descriptionFunc?: (arg: string) => React.ReactNode;
}) {
  const [value, setValue] = useState(initialValue ?? '');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const [touched, setTouched] = useState<boolean>(false);
  const [typeModified, setTypeModified] = useState<string>(type); //in case we have password field which can switch between "text" and "password"
  const formRef = useRef<HTMLFormElement>(null);

  const validate = useCallback(
    async (value: string, submit: boolean) => {
      if (required && !value && submit) throw new Error('Cannot be empty');
      if (type === 'number' || isFloat) {
        if (!value && submit) {
          throw new Error('Cannot be empty');
        }
        const numValue = isFloat
          ? parseFloatStrict(value)
          : parseintStrict(value);
        if (isNaN(numValue)) throw new Error('Must be a number');
        if (min && numValue < min)
          throw new Error(`Must be greater than ${min}`);
        if (max && numValue > max) throw new Error(`Must be less than ${max}`);
      }
      if (validateProp) await validateProp(value);
    },
    [max, min, type, validateProp, required, isFloat]
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
    setTouched(initialValue != value);
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

  const errorText = errorProp || error;
  disabled = disabled || !canRead || isLoadingProp;
  description = canRead
    ? descriptionFunc?.(initialValue || value) ?? description
    : 'No permission';

  let icons = [];

  if (typeModified !== 'password' && touched) {
    icons.push(
      <FontAwesomeIcon
        icon={faFloppyDisk}
        className="w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500"
        onClick={() => {
          formRef.current?.requestSubmit();
          setTypeModified('password');
        }}
        key="save"
      />
    );
    icons.push(
      <FontAwesomeIcon
        icon={faRotateRight}
        className="w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500"
        onClick={() => formRef.current?.reset()}
        key="reset"
      />
    );
  }
  if (type === 'password') {
    icons.push(
      <FontAwesomeIcon
        icon={typeModified === 'password' ? faEye : faEyeSlash}
        className="w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500"
        onClick={() => {
          typeModified === 'password'
            ? setTypeModified('text')
            : setTypeModified('password');
          formRef.current?.reset();
        }}
        key="reveal password"
      />
    );
  }
  if (isLoading || isLoadingProp) {
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
      className={`flex flex-row items-center justify-between ${className} group relative gap-4 bg-gray-800 px-4 py-3 text-medium`}
    >
      <div className={`flex min-w-0 grow flex-col`}>
        <label className="text-medium font-medium text-gray-300">{label}</label>
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
      <form
        onSubmit={onSubmit}
        onReset={onReset}
        className="relative w-5/12 flex-shrink-0"
        ref={formRef}
        onKeyDown={handleKeyDown}
        id={id}
      >
        <div
          className={`pointer-events-none absolute top-0 right-0 flex h-full flex-row items-center justify-end py-1.5 px-3`}
        >
          <div className="pointer-events-auto flex flex-row gap-2">
            {showIcons && icons}
          </div>
        </div>
        <input
          value={typeModified === 'password' ? 'xxxxxxxx' : value}
          placeholder={placeholder}
          onChange={onChange}
          maxLength={maxLength}
          style={{ paddingRight: `${icons.length * 1.5 + 0.75}rem` }} //0.75, 2.25, 3.75
          className={`input-shape input-background input-outlines input-text-style w-full ${
            errorText ? 'input-border-error' : 'input-border-normal'
          }
           `}
          onBlur={() => {
            setValue(value.trim());
          }}
          disabled={typeModified === 'password' ? true : disabled}
          type={typeModified}
          autoComplete={DISABLE_AUTOFILL}
        />
      </form>
    </div>
  );
}
