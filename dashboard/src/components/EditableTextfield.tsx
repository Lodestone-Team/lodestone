import { faFloppyDisk, faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import clsx from 'clsx';
import React, { useEffect, useState } from 'react';
import { BeatLoader } from 'react-spinners';
import { catchAsyncToString } from 'utils/util';
import AutoGrowInput from './Atoms/AutoGrowInput';

export type TextfieldType = 'heading' | 'description';

type Props = {
  initialText: string;
  type?: TextfieldType;
  containerClassName?: string;
  textClassName?: string;
  iconClassName?: string;
  placeholder?: string;
  onSubmit: (arg: string) => Promise<void>;
};

export default function EditableTextfield({
  initialText,
  type = 'heading',
  containerClassName = '',
  textClassName = '',
  iconClassName = '',
  placeholder = '',
  onSubmit: onSubmitProp,
}: Props) {
  const [displayText, setDisplayText] = useState<string>(initialText);
  const [editText, setEditText] = useState<string>(initialText);

  const [isEditing, setIsEditing] = useState<boolean>(false);

  const [isLoading, setIsLoading] = useState<boolean>(false);

  const [error, setError] = useState<string>('');

  const onEdit = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setEditText(currentText);
  };

  const onSave = async () => {
    setIsLoading(true);
    const trimmed = editText.trim();
    setEditText(trimmed);
    setIsLoading(true);
    const error = await catchAsyncToString(onSubmitProp(trimmed));
    console.log(error);
    setError(error);
    setIsLoading(false);
    if (!error) setDisplayText(trimmed);
  };

  const onCancel = () => {
    if (isLoading) return;
    setEditText(displayText);
    setIsEditing(false);
    setError('');
  };

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (!isEditing) return;
      if (e.code === 'Enter') {
        onSave();
      } else if (e.code === 'Escape') {
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKey);

    return () => {
      window.removeEventListener('keydown', handleKey);
    };
  });

  const errorNode = error ? (
    <div
      className={clsx(
        `absolute whitespace-nowrap text-right font-sans not-italic text-red-200`,
        type === 'heading' &&
          'top-[-1.5em] text-medium font-medium tracking-normal',
        type === 'description' && 'bottom-[-1.3em] text-small'
      )}
    >
      {error}
    </div>
  ) : null;

  return (
    <div
      className={`group relative flex flex-row items-center justify-start gap-1 ${
        type === 'heading'
          ? 'dashboard-instance-heading'
          : 'text-h3 font-medium italic tracking-tight'
      } ${containerClassName}`}
    >
      <div
        className={`mr-[0.5ch] min-w-0  ${
          type === 'heading' ? 'rounded-lg' : 'rounded'
        }`}
      >
        {isEditing ? (
          <AutoGrowInput
            textClassName={clsx(
              `bg-transparent focus:outline-none`,
              error ? 'text-red-200' : 'text-gray-300',
              textClassName
            )}
            value={editText}
            onChange={onEdit}
            onBlur={onCancel}
            autoFocus={true}
            placeholder={placeholder}
          />
        ) : (
          <div
            className={`
          ${
            type === 'heading'
              ? 'text-gray-300 decoration-2 underline-offset-[6px]'
              : 'text-gray-500'
          } 
          ml-[-1ch] mr-[-0.5ch] truncate bg-transparent group-hover:text-gray-300 group-hover:underline ${textClassName}`}
            onClick={() => {
              setIsEditing(true);
            }}
          >
            <span
              className={`whitespace-pre bg-transparent pl-[1ch] pr-[0.5ch]`}
            >
              {displayText ? displayText : placeholder}
            </span>
          </div>
        )}
      </div>
      {errorNode}
      {isLoading ? (
        <BeatLoader
          size={`${type === 'heading' ? '0.5rem' : '0.25rem'}`}
          cssOverride={{
            width: `${type === 'heading' ? '3rem' : '2rem'}`,
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'center',
            margin: `0 -0.5rem`,
          }}
          color="#6b7280"
        />
      ) : (
        <FontAwesomeIcon
          className={clsx(
            `text-gray-faded/30 hover:cursor-pointer group-hover:text-gray-500`,
            type === 'heading' && 'w-6',
            type === 'description' && 'w-4',
            iconClassName
          )}
          icon={isEditing ? faFloppyDisk : faPenToSquare}
          onMouseDown={(e) => {
            if (isEditing) e.preventDefault();
          }}
          onClick={() => {
            if (isEditing) {
              onSave();
            } else {
              setIsEditing(true);
            }
          }}
        />
      )}
    </div>
  );
}
