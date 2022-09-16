import { faFloppyDisk, faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import React, { useEffect, useState } from 'react';
import { BeatLoader } from 'react-spinners';
import CircleLoader from 'react-spinners/CircleLoader';

export type TextfieldType = 'heading' | 'description';

type Props = {
  initialText: string;
  type?: TextfieldType;
  containerClassName?: string;
  textClassName?: string;
  iconClassName?: string;
  onSubmit?: (arg: string) => void;
};

export default function EditableTextfield({
  initialText,
  type = 'heading',
  containerClassName,
  textClassName,
  iconClassName,
  onSubmit = () => {
    //
  },
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
    await new Promise((resolve) => {
      setTimeout(resolve, 1000);
    });
    try {
      await onSubmit(editText);
    } finally {
      setIsLoading(false);
    }
    setDisplayText(editText);
    setIsEditing(false);
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
      className={`flex flex-row justify-between items-center space-x-2 tracking-tight ${
        type === 'heading' ? 'font-semibold font-heading text-xlarge' : 'italic'
      } ${containerClassName}`}
    >
      {isLoading ? (
        // <div className={`${type === 'heading' ? 'h-8' : 'h-4'}`}>
          <BeatLoader size={`${type === 'heading' ? '0.5rem' : '0.25rem'}`} color="#6b7280" />
        // </div>
      ) : isEditing ? (
        <FontAwesomeIcon
          className={`${iconClassName} text-gray-500 ${
            type === 'heading' ? 'h-8' : 'h-4'
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
            type === 'heading' ? 'h-8' : 'h-4'
          } ${iconClassName}`}
          icon={faPenToSquare}
          onClick={() => {
            setIsEditing(true);
          }}
        />
      )}
      {isEditing ? (
        <input
          className={`bg-transparent text-gray-300 flex-1 tracking-tight focus:outline-none ${textClassName}`}
          value={editText}
          onChange={onEdit}
          onBlur={onCancel}
          autoFocus={true}
        />
      ) : (
        <span
          className={`bg-transparent text-gray-300 flex-1 truncate hover:underline ${textClassName}`}
        >
          {displayText}
        </span>
      )}
    </div>
  );
}
