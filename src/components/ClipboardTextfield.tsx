import { faClone } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Tooltip from 'rc-tooltip';
import Label from './Atoms/Label';
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
      <Label
        size="large"
        color="gray"
        className="group flex flex-row items-center gap-3 hover:cursor-pointer"
        onClick={onClickCopy}
      >
        <div className={`select-none ${className}`}>
          {' '}
          {/* TODO develop custom tooltip component */}
          {text}&nbsp;&nbsp;
          <FontAwesomeIcon
            className={`text-gray-faded/30 group-hover:text-gray-500`}
            icon={faClone}
          />
        </div>
      </Label>
    </Tooltip>
  );
}
