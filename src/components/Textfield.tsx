import { faFloppyDisk } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { HTMLInputTypeAttribute, useEffect, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { catchAsyncToString } from 'utils/util';

const onChangeValidateTimeout = 100;
const inputClassName =
  'appearance-none bg-gray-700 p-1.5 rounded-md  enabled:outline enabled:outline-2 enabled:text-gray-300 tracking-tight leading-snug font-medium enabled:focus-visible:ring-[6px]  disabled:text-gray-500 disabled:bg-gray-800';
const inputBorderClassName =
  'enabled:outline-gray-400 enabled:focus-visible:outline-blue enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

export default function Textfield({
  label,
  value: initialValue,
  className,
  onSubmit: onSubmitProp,
  type,
  min,
  max,
  removeArrows,
  disabled = false,
  validate, //throws error if invalid
}: {
  label: string;
  value: string;
  className?: string;
  type?: HTMLInputTypeAttribute;
  min?: number;
  max?: number;
  removeArrows?: boolean;
  disabled?: boolean;
  onSubmit: (arg: string) => Promise<void>;
  validate?: (arg: string) => Promise<void>;
}) {
  const [value, setValue] = useState(initialValue);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const [touched, setTouched] = useState<boolean>(false);

  // we want to validate the input after the user stops typing for a while
  useEffect(() => {
    if (!touched) return;
    const timeout = setTimeout(async () => {
      if (validate) {
        const trimmed = value.trim();
        const error = await catchAsyncToString(validate(trimmed));
        setError(error);
        if (error) return;
      }
    }, onChangeValidateTimeout);
    return () => clearTimeout(timeout);
  }, [value, validate, touched]);

  // set touch to false when the value changes
  useEffect(() => {
    setTouched(initialValue !== value);
    if (initialValue !== value) setError('');
  }, [initialValue, value]);

  const onChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setValue(currentText);
    setTouched(true);
  };

  const onSubmit = async () => {
    if (!touched) return;
    const trimmed = value.trim();
    setValue(trimmed);
    setIsLoading(true);
    if (validate) {
      const error = await catchAsyncToString(validate(value));
      setError(error);
      setIsLoading(false);
      if (error) return;
    }
    const error = await catchAsyncToString(onSubmitProp(trimmed));
    setError(error);
    setIsLoading(false);
  };

  const onFormSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    await onSubmit();
  };

  let icon = null;
  if (isLoading) {
    icon = (
      <BeatLoader
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
    );
  }
  if (touched) {
    icon = (
      <FontAwesomeIcon
        icon={faFloppyDisk}
        className="w-4 text-gray-faded/30 hover:cursor-pointer hover:text-gray-500"
        onClick={onSubmit}
      />
    );
  }

  return (
    <div className={`flex flex-col gap-1 ${className} group relative`}>
      <label className="block font-medium text-gray-300 text-small">
        {label}:
      </label>
      <div className="mt-1">
        <form onSubmit={onFormSubmit} className="relative">
          <div className="absolute top-0 right-0 flex h-full flex-row items-center justify-end p-1.5">
            {icon}
          </div>
          <input
            type={type}
            value={value}
            onChange={onChange}
            className={`${inputClassName} ${
              error ? inputErrorBorderClassName : inputBorderClassName
            }
            ${removeArrows && 'noSpin'}`}
            onBlur={() => {
              setValue(value.trim());
            }}
            min={min}
            max={max}
            disabled={disabled}
          />
        </form>
        {error && (
          <div
            className={`absolute -bottom-6 whitespace-nowrap text-right font-sans text-small not-italic text-red
          `}
          >
            {error || 'Unknown error'}
          </div>
        )}
      </div>
    </div>
  );
}
