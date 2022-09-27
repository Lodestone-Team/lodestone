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
    const trimmedText = editText.trim()
    await new Promise((resolve) => {
      setTimeout(resolve, 1000);
    });
    try {
      const { status, message } = await onSubmit(trimmedText);
      setErrorStatus(status);
      if (status) {
        setErrorMessage(message);
        setIsEditing(true);
      } else {
        setIsEditing(false);
      }
    } finally {
      setIsLoading(false);
      setDisplayText(trimmedText);
      setEditText(trimmedText);
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

  const errorNode = errorStatus ? (
    <div
      className={`absolute text-right font-sans not-italic text-red ${
        type === 'heading'
          ? 'text-base font-normal tracking-normal -bottom-[1.3em] left-10'
          : 'text-small -bottom-[1.3em] left-6'
      }`}
    >
      {errorMessage}
    </div>
  ) : null;

  const iconSize = type === 'heading' ? 'w-8' : 'w-4';

  return (
    <div
      className={`relative flex flex-row justify-start items-center gap-2 tracking-tight group ${
        type === 'heading' ? 'font-semibold font-heading text-xlarge' : 'italic'
      } ${containerClassName}`}
    >
      {isLoading ? (
        <BeatLoader
          size={`${type === 'heading' ? '0.5rem' : '0.25rem'}`}
          cssOverride={{
            width: `${type === 'heading' ? '3rem' : '2rem'}`,
            // negative padding to give it extra space
            margin: `0 -0.5rem`,
          }}
          color="#6b7280"
        />
      ) : (
        <FontAwesomeIcon
          className={`text-gray-faded/30 group-hover:text-gray-500 hover:cursor-pointer ${iconSize} ${iconClassName}`}
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

      {isEditing ? (
        <AutoGrowInput
          className={`
          ml-[-0.25rem] pl-1
          ${type === 'heading' ? 'rounded-lg' : 'rounded'} 
          ${errorStatus ? `border-2 border-red` : ''}`}
          textClassName={`focus:outline-none tracking-tight bg-transparent text-gray-300 ${textClassName}`}
          value={editText}
          onChange={onEdit}
          onBlur={onCancel}
          autoFocus={true}
        ></AutoGrowInput>
      ) : (
        <span
          className={`
          ml-[-0.25rem] pl-1 pr-[0.50ch]
          ${type === 'heading' ? 'rounded-lg' : 'rounded'} 
          ${errorStatus ? `border-2 border-red` : ''}
          bg-transparent text-gray-300 truncate group-hover:underline ${textClassName}`}
          onClick={() => {
            setIsEditing(true);
          }}
        >
          {displayText}
        </span>
      )}
      {errorNode}
    </div>
  );
}
