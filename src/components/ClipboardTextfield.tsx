import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Tooltip from 'rc-tooltip';
import { toast } from 'react-toastify';
import clsx from 'clsx';
import { LabelColor } from './Atoms/Label';

export default function ClipboardTextfield({
  color = 'gray',
  text,
  textToCopy,
  className,
}: {
  text: string;
  textToCopy?: string;
  className?: string;
  color?: LabelColor;
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
        className={`group flex flex-row items-center justify-center gap-3 whitespace-nowrap hover:cursor-pointer ${className}`}
        onClick={onClickCopy}
      >
        <div className={`select-none`}>
          {/* TODO develop custom tooltip component */}
          {text}&nbsp;&nbsp;
          <FontAwesomeIcon
            className={clsx({
              gray: `mr-2 text-gray-faded/25 group-hover:text-gray-500`,
              blue: `mr-2 text-blue-faded/25 group-hover:text-blue-150`,
              green: 'mr-2 text-green-faded/25 group-hover:text-green',
              yellow: 'mr-2 text-yellow-faded/25 group-hover:text-yellow',
              red: 'mr-2 text-red-faded/25 group-hover:text-red',
            }[color])}
            icon={faClone}
          />
        </div>
      </div>
    </Tooltip>
  );
}
