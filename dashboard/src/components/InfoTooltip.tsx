import type React from "react";
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faInfoCircle } from '@fortawesome/free-solid-svg-icons';
import Tooltip from 'rc-tooltip';
import clsx from 'clsx';


const InfoTooltip = ({
  className,
  tooltip,
} : { 
  className?: string,
  tooltip: string,
}) => {
  return (
    <>
      <Tooltip
        overlay={<span>{tooltip}</span>}
        placement="top"
        showArrow={false}
        trigger={['hover']}
        mouseEnterDelay={0}
      >
        <FontAwesomeIcon
          icon={faInfoCircle}
          className={clsx(className, "mt-[3px] text-gray-400")}
        />
      </Tooltip>
    </>
  )
}

export default InfoTooltip;
