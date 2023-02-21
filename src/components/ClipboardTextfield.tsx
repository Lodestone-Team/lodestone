import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Tooltip from 'rc-tooltip';
import { toast } from 'react-toastify';

export default function ClipboardTextfield({
  text,
  textToCopy,
  className,
}: {
  text: string;
  textToCopy?: string;
  className?: string;
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
          <FontAwesomeIcon
            className={`mr-2 text-gray-faded/30 group-hover:text-gray-500`}
            icon={faClone}
          />
          {/* TODO develop custom tooltip component */}
          {text}&nbsp;&nbsp;
        </div>
      </div>
    </Tooltip>
  );
}
