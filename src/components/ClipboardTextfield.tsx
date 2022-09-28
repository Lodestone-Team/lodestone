import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useRef, useState } from 'react';
import ToolTip from './ToolTip';

export default function ClipboardTextfield({
  text,
  textToCopy,
  className,
}: {
  text: string;
  textToCopy?: string;
  className?: string;
}) {
  const onClickCopy = (e: React.MouseEvent<HTMLDivElement>) => {
    e.stopPropagation();
    navigator.clipboard.writeText(textToCopy || text);
    //TODO: toast "copied" when we have notifications setup
    alert(`Copied "${textToCopy || text}"`);
  };

  return (
    <div
      className={`hover:cursor-pointer select-none group ${className}`}
      onClick={onClickCopy}
      title="Click to Copy"
    > {/* TODO develop custom tooltip component */}
      {text}&nbsp;&nbsp;
      <FontAwesomeIcon className={`text-gray-faded/30 group-hover:text-gray-500`} icon={faClone} />
    </div>
  );
}
