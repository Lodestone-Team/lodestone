import { faFloppyDisk, faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import React, { useEffect, useState } from 'react';
import FadeLoader from "react-spinners/FadeLoader"

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
  type = "heading",
  containerClassName,
  textClassName,
  iconClassName,
  onSubmit = () => {
    //
  },
}: Props) {
  const [displayText, setDisplayText] = useState(initialText);
  const [editText, setEditText] = useState(initialText);

  const [isEditing, setIsEditing] = useState(false);

  const [isLoading, setIsLoading] = useState(false);

  const onEdit = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setEditText(currentText);
  };

  const onSave = async () => {
    setIsLoading(true);
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
      className={`flex flex-row justify-between items-center space-x-2 tracking-tight ${type === "heading" ? "font-semibold font-heading text-xlarge" : "italic"} ${containerClassName}`}
    >
      {isEditing ? (
        <>
          <FontAwesomeIcon
            className={`${iconClassName} text-gray-500 ${type === "heading" ? "h-8" : "h-4"}`}
            icon={faFloppyDisk}
            onMouseDown={(e) => {e.preventDefault()}}
            onClick={onSave}
          />
          <input
            className={`bg-transparent text-gray-300 flex-1 tracking-tight focus:outline-none ${textClassName}`}
            value={editText}
            onChange={onEdit}
            onBlur={onCancel}
            autoFocus={true}
          />
        </>
      ) : (
        <>
          <FontAwesomeIcon
            className={`text-gray-500 ${type === "heading" ? "h-8" : "h-4"} ${iconClassName}`}
            icon={faPenToSquare}
            onClick={() => {
              setIsEditing(true);
            }}
          />
          <span
            className={`bg-transparent text-gray-300 flex-1 truncate hover:underline ${textClassName}`}
          >
            {displayText}
          </span>
        </>
      )}
    </div>
  );
}
