import {
  autoUpdate,
  flip,
  offset,
  Placement,
  shift,
  useFloating,
} from '@floating-ui/react-dom';
import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useRef, useState } from 'react';
import ToolTip from './ToolTip';

export default function ClipboardTextfield({
  text,
  copyText,
  className,
  iconClassName,
  placement = 'top',
}: {
  text: string;
  copyText?: string;
  className?: string;
  iconClassName?: string;
  placement?: Placement;
}) {
  const onClickCopy = (e: React.MouseEvent<HTMLDivElement>) => {
    e.stopPropagation();
    navigator.clipboard.writeText(copyText || text);
    //TODO: toast "copied" when we have notifications setup
    alert(`Copied "${copyText || text}"`);
  };

  return (
    <div
      className={`${className} hover:cursor-pointer w-fit`}
      onClick={onClickCopy}
      title="Click to Copy"
    > {/* TODO develop custom tooltip component */}
      {text}
      {'  '}
      <FontAwesomeIcon className={`${iconClassName}`} icon={faClone} />
    </div>
  );
}
