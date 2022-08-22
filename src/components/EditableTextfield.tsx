import { faFloppyDisk, faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import React, { useEffect, useState } from 'react';
import { ReactReduxContextInstance } from 'react-redux/es/components/Context';

type Props = {
  initialText: string;
  textClassName?: string;
  iconClassName?: string;
  onSubmit?: Function;
};

export default function EditableTextfield({
  initialText,
  textClassName,
  iconClassName,
  onSubmit = () => {},
}: Props) {
  const [displayText, setDisplayText] = useState(initialText);
  const [editText, setEditText] = useState(initialText);

  const [isEditing, setIsEditing] = useState(false);

  const onEdit = (e: React.ChangeEvent<HTMLInputElement>) => {
    const currentText = e.target.value;
    setEditText(currentText);
  };

  const onSave = () => {
    setDisplayText(editText);
    onSubmit(editText);
    setIsEditing(false);
  };

  const onCancel = () => {
    setEditText(displayText);
    setIsEditing(false);
  };

  useEffect(() => {
    const handleKey = (e: any) => {
      if (e.code === "Enter") {
        onSave();
      }
      else if (e.code === "Escape") {
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKey);

    return () => {
      window.removeEventListener('keydown', handleKey);
    };
  });

  return (
    <div>
      {isEditing ? (
        <div>
          <input
            className={`${textClassName}`}
            placeholder={displayText}
            onChange={onEdit}
            onBlur={onCancel}
            autoFocus={true}
          />
          <FontAwesomeIcon
            className={`${iconClassName} text-gray-500`}
            icon={faFloppyDisk}
            onMouseDown={(e) => [e.preventDefault()]}
            onClick={onSave}
          />
        </div>
      ) : (
        <div>
          <span className={`${textClassName}`}>{displayText}</span>
          <FontAwesomeIcon
            className={`${iconClassName} text-gray-500`}
            icon={faPenToSquare}
            onClick={() => {
              setIsEditing(true);
            }}
          />
        </div>
      )}
    </div>
  );
}
