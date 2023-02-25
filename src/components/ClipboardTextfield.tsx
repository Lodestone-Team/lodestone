import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Tooltip from 'rc-tooltip';
import { toast } from 'react-toastify';
import clsx from 'clsx';
import { LabelColor } from './Atoms/Label';

export default function ClipboardTextfield({
  color = 'gray',
  iconLeft = true,
  text,
  textToCopy,
  className,
}: {
  text: string;
  textToCopy?: string;
  className?: string;
  color?: LabelColor;
  iconLeft?: boolean;
}) {
  const onClickCopy = (e: React.MouseEvent<HTMLSpanElement>) => {
    e.stopPropagation();
    // note that navigator.clipboard.writeText is only supported over HTTPS
    navigator.clipboard.writeText(textToCopy || text);
    toast.info(`Copied "${textToCopy || text}"`);
  };

  return (
    <Tooltip
      showArrow={false}
      overlay={<span>Copy to clipboard</span>}
      placement="bottom"
      trigger={['hover']}
      mouseEnterDelay={0.2}
    >
      <div
        className={`group flex flex-row items-center justify-center gap-2 whitespace-nowrap hover:cursor-pointer ${className} select-none`}
        onClick={onClickCopy}
      >
          {/* TODO develop custom tooltip component */}
          {!iconLeft && <>{text}</>}
          <FontAwesomeIcon
            className={clsx({
              gray: `text-gray-faded/25 group-hover:text-gray-500`,
              blue: `text-blue-faded/25 group-hover:text-blue-150`,
              green: 'text-green-faded/25 group-hover:text-green',
              yellow: 'text-yellow-faded/25 group-hover:text-yellow',
              red: 'text-red-faded/25 group-hover:text-red',
            }[color],
            )}
            icon={faClone}
          />
          {iconLeft && <>{text}&nbsp;&nbsp;</>}
      </div>
    </Tooltip>
  );
}
