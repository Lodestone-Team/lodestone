import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useRef, useState } from 'react';
import ToolTip from './ToolTip';

export default function ClipboardTextfield({
  text,
  textToCopy,
  className,
  iconClassName,
}: {
  text: string;
  textToCopy?: string;
  className?: string;
  iconClassName?: string;
}) {
  const onClickCopy = (e: React.MouseEvent<HTMLDivElement>) => {
    e.stopPropagation();
    navigator.clipboard.writeText(textToCopy || text);
    //TODO: toast "copied" when we have notifications setup
    alert(`Copied "${textToCopy || text}"`);
  };

  return (
    <div
      className={`${className} hover:cursor-pointer w-fit`}
      onClick={onClickCopy}
      title="Click to Copy"
    > {/* TODO develop custom tooltip component */}
      {text}&nbsp;&nbsp;
      <FontAwesomeIcon className={`${iconClassName} text-gray-500`} icon={faClone} />
    </div>
  );
}
