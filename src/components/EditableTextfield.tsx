import { faFloppyDisk, faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import React, { useEffect, useState } from 'react';
import { BeatLoader } from 'react-spinners';
import AutoGrowInput from './AutoGrowInput';

export type TextfieldType = 'heading' | 'description';

type Props = {
  initialText: string;
  type?: TextfieldType;
  containerClassName?: string;
  textClassName?: string;
  iconClassName?: string;
  onSubmit: (arg: string) => { status: number; message: string };
};

export default function EditableTextfield({
  initialText,
  type = 'heading',
  containerClassName = '',
  textClassName = '',
  iconClassName = '',
  onSubmit,
}: Props) {
  const [displayText, setDisplayText] = useState<string>(initialText);
  const [editText, setEditText] = useState<string>(initialText);

  const [isEditing, setIsEditing] = useState<boolean>(false);

  const [isLoading, setIsLoading] = useState<boolean>(false);

  const [errorStatus, setErrorStatus] = useState<number>(0);
  const [errorMessage, setErrorMessage] = useState<string>('');

  const onEdit = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setEditText(currentText);
  };

  const onSave = async () => {
    setIsLoading(true);
    await new Promise((resolve) => {
      setTimeout(resolve, 1000);
    });
    try {
      const { status, message } = await onSubmit(editText);
      setErrorStatus(status);
      if (status) {
        setErrorMessage(message);
      }
    } finally {
      setIsLoading(false);
      setDisplayText(editText);
      setIsEditing(false);
    }
  };

  const onCancel = () => {
    setEditText(displayText);
    setIsEditing(false);
  };

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
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

  return (
    <div
      className={`flex flex-row justify-start items-center gap-2 tracking-tight ${
        type === 'heading' ? 'font-semibold font-heading text-xlarge' : 'italic'
      } ${containerClassName}`}
    >
      {isLoading ? (
        // <div className={`${type === 'heading' ? 'h-8' : 'h-4'}`}>
        <BeatLoader
          size={`${type === 'heading' ? '0.5rem' : '0.25rem'}`}
          cssOverride={{
            width: `${type === 'heading' ? '3rem' : '2rem'}`,
            // negative padding to give it extra space
            margin: `0 -0.5rem`,
          }}
          color="#6b7280"
        />
      ) : // </div>
      isEditing ? (
        <FontAwesomeIcon
          className={`${iconClassName} text-gray-500 ${
            type === 'heading' ? 'w-8' : 'w-4'
          }`}
          icon={faFloppyDisk}
          onMouseDown={(e) => {
            e.preventDefault();
          }}
          onClick={onSave}
        />
      ) : (
        <FontAwesomeIcon
          className={`text-gray-500 ${
            type === 'heading' ? 'w-8' : 'w-4'
          } ${iconClassName}`}
          icon={faPenToSquare}
          onClick={() => {
            setIsEditing(true);
          }}
        />
      )}

      {isEditing ? (
        <AutoGrowInput
          className={`bg-transparent text-gray-300 tracking-tight focus:outline-none ${textClassName} ${
            errorStatus
              ? `border-2 ${
                  type === 'heading' ? 'rounded-xl' : 'rounded'
                }  border-red`
              : ''
          }`}
          value={editText}
          onChange={onEdit}
          onBlur={onCancel}
          autoFocus={true}
        />
      ) : (
        <span
          className={`bg-transparent text-gray-300 truncate hover:underline ${textClassName} ${
            errorStatus
              ? `border-2 ${
                  type === 'heading' ? 'rounded-xl' : 'rounded'
                }  border-red`
              : ''
          }`}
          onClick={() => {
            setIsEditing(true);
          }}
        >
          {displayText}
        </span>
      )}
      {errorStatus ? (
        <div
          className={`absolute font-sans not-italic	text-red ${
            type === 'heading'
              ? 'text-base font-normal tracking-normal top-24 left-12'
              : 'text-small top-12 left-8'
          }`}
        >
          {errorMessage}
        </div>
      ) : (
        <></>
      )}
    </div>
  );
}
