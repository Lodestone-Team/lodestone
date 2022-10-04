import { faFloppyDisk } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useState } from 'react';
import BeatLoader from 'react-spinners/BeatLoader';
import { Result } from '@badrap/result';
import { ok } from 'assert';
import { ClientError } from 'data/ClientError';

const onChangeValidateTimeout = 500;
const inputClassName =
  'bg-gray-700 p-1.5 rounded-md  enabled:outline enabled:outline-2 enabled:text-gray-300 tracking-tight leading-snug font-medium enabled:focus-visible:ring-[6px]  disabled:text-gray-600 disabled:bg-gray-800';
const inputBorderClassName =
  'enabled:outline-gray-400 enabled:focus-visible:outline-blue enabled:focus-visible:ring-blue/30 invalid:outline-red invalid:focus-visible:outline-red';
const inputErrorBorderClassName =
  'outline-red focus-visible:outline-red enabled:focus-visible:ring-red-faded/30';

export default function Textfield({
  label,
  value: initialValue,
  className,
  onSubmit: onSubmitProp,
  validate,
}: {
  label: string;
  value: string;
  className?: string;
  onSubmit: (arg: string) => Promise<Result<void, ClientError>>;
  validate?: (arg: string) => Promise<Result<void, ClientError>>;
}) {
  const [value, setValue] = useState(initialValue);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [touched, setTouched] = useState<boolean>(false);

  // we want to validate the input after the user stops typing for a while
  useEffect(() => {
    const timeout = setTimeout(async () => {
      if (validate) {
        setIsLoading(true);
        const result = await validate(value);
        result.unwrap(
          (_) => {
            setIsLoading(false);
            setError(false);
            setErrorMessage('');
          },
          (error) => {
            setIsLoading(false);
            setError(true);
            setErrorMessage(error.detail);
          }
        );
      }
    }, onChangeValidateTimeout);
    return () => clearTimeout(timeout);
  }, [value, validate]);

  // set touch to false when the value changes
  useEffect(() => {
    setTouched(false);
  }, [value]);

  const onChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setValue(currentText);
    setTouched(true);
  };

  const onSubmit = async () => {
    setValue(value.trim());
    setIsLoading(true);
    const result = await onSubmitProp(value.trim());
    result.unwrap(
      (_) => {
        setIsLoading(false);
        setError(false);
        setErrorMessage('');
      },
      (error) => {
        setIsLoading(false);
        setError(true);
        setErrorMessage(error.detail);
      }
    );
  };

  const onFormSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    await onSubmit();
  };

  return (
    <div className={`flex flex-col gap-1 ${className} group relative`}>
      <label className="block font-medium text-gray-300 text-small">
        {label}:
      </label>
      <div className="mt-1">
        <form onSubmit={onFormSubmit} className="relative">
          <div className="absolute top-0 right-0 flex flex-row items-center justify-end h-full p-1.5">
            {isLoading ? (
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
            ) : (
              <FontAwesomeIcon
                icon={faFloppyDisk}
                spin={isLoading}
                className="w-4 text-gray-faded/30 hover:text-gray-500 hover:cursor-pointer"
                onClick={onSubmit}
              />
            )}
          </div>
          <input
            type="text"
            value={value}
            onChange={onChange}
            className={`${inputClassName} ${
              error ? inputErrorBorderClassName : inputBorderClassName
            }`}
            onBlur={() => {
              setValue(value.trim());
            }}
          />
        </form>
        {error && (
          <div
            className={`absolute whitespace-nowrap text-right font-sans not-italic text-red text-smaller -bottom-5
          `}
          >
            {errorMessage || 'Unknown error'}
          </div>
        )}
      </div>
    </div>
  );
}
