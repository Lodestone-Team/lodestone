import { faFloppyDisk, faRotateRight } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  HTMLInputTypeAttribute,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';

const onChangeValidateTimeout = 100;
const inputClassName =
  'w-full appearance-none bg-gray-700 p-1.5 rounded-md  enabled:outline enabled:outline-2 enabled:text-gray-300 tracking-tight leading-snug font-medium enabled:focus-visible:ring-[6px]  disabled:text-gray-500 disabled:bg-gray-800';
const inputBorderClassName =
  'enabled:outline-gray-400 enabled:focus-visible:outline-blue enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

export type TextFieldType = 'text' | 'number';

export default function Textfield({
  label,
  value: initialValue,
  className,
  onSubmit: onSubmitProp,
  type = 'text',
  min,
  max,
  error: errorProp,
  removeArrows,
  disabled = false,
  validate: validateProp, //throws error if invalid
}: {
  label: string;
  value: string;
  className?: string;
  type?: TextFieldType;
  min?: number;
  max?: number;
  error?: string;
  removeArrows?: boolean;
  disabled?: boolean;
  onSubmit: (arg: string) => Promise<void>;
  validate?: (arg: string) => Promise<void>;
}) {
  const [value, setValue] = useState(initialValue);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const [touched, setTouched] = useState<boolean>(false);
  const formRef = useRef<HTMLFormElement>(null);

  const validate = useCallback(
    async (value: string) => {
      if (type === 'number') {
        if(!value) {
          throw new Error('Cannot be empty');
        }
        const numValue = parseInt(value);
        if (isNaN(numValue)) throw new Error('Must be a number');
        if (min !== undefined && numValue < min)
          throw new Error(`Must be greater than ${min}`);
        if (max !== undefined && numValue > max)
          throw new Error(`Must be less than ${max}`);
      }
      if (validateProp) await validateProp(value);
    },
    [max, min, type, validateProp]
  );

  // we want to validate the input after the user stops typing for a while
  useEffect(() => {
    if (!touched) return;
    const timeout = setTimeout(async () => {
      const trimmed = value.trim();
      const error = await catchAsyncToString(validate(trimmed));
      setError(error);
      if (error) return;
    }, onChangeValidateTimeout);
    return () => clearTimeout(timeout);
  }, [value, validate, touched]);

  // set touch to false when the value changes
  useEffect(() => {
    setTouched(initialValue !== value);
    if (initialValue !== value) setError('');
  }, [initialValue, value]);

  // set value to initialValue when initialValue changes
  useEffect(() => {
    setValue(initialValue);
  }, [initialValue]);

  const onChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setValue(currentText);
    setTouched(true);
  };

  const onSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!touched) return;
    const trimmed = value.trim();
    setValue(trimmed);
    setIsLoading(true);
    const validateError = await catchAsyncToString(validate(value));
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
    setValue(initialValue);
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
        className="w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500"
        onClick={() => formRef.current?.requestSubmit()}
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
  if (isLoading) {
    icons = [(
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
      />
    )];
  }

  return (
    <div className={`flex flex-col gap-1 ${className} group relative`}>
      <label className="block font-medium text-gray-300 text-small">
        {label}:
      </label>
      <div className="mt-1">
        <form
          onSubmit={onSubmit}
          onReset={onReset}
          className="relative"
          ref={formRef}
          onKeyDown={handleKeyDown}
        >
          <div className="absolute top-0 right-0 flex h-full flex-row items-center justify-end p-1.5">
            <div className="flex flex-row gap-2">{icons}</div>
          </div>
          <input
            value={value}
            onChange={onChange}
            className={`${inputClassName} ${
              uiError ? inputErrorBorderClassName : inputBorderClassName
            }
            ${removeArrows && 'noSpin'}`}
            onBlur={() => {
              setValue(value.trim());
            }}
            disabled={disabled}
          />
        </form>
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
