import type React from "react";
import { useState } from "react";
import { usePopper } from "react-popper";
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faInfoCircle } from '@fortawesome/free-solid-svg-icons';
import clsx from 'clsx';


const Tooltip = ({
  className,
  tooltip,
} : { 
  className?: string,
  tooltip: string,
}) => {
  
  const [isOpen, setIsOpen] = useState(false);
  const [referenceElement, setReferenceElement] = useState<HTMLDivElement | null>(null);
  const [popperElement, setPopperElement] = useState<HTMLDivElement | null>(null);
  const { styles, attributes } = usePopper(referenceElement, popperElement, {
    placement: "top",
  });
  
  return (
    <div>
      <div
          ref={setReferenceElement}
          onMouseEnter={() => setIsOpen(true)}
          onMouseLeave={() => setIsOpen(false)}
          className="relative inline-block w-auto"
      >
        <FontAwesomeIcon
          icon={faInfoCircle}
          className={clsx(className, "text-gray-400")}
        />
      </div>

      <div
        ref={setPopperElement}
        style={styles.popper}
        {...attributes.popper}
        className={`relative rounded-md bg-gray-900 text-white shadow-lg max-w-xs p-2 transition-opacity ${isOpen ? "opacity-100" : "opacity-0"}`}
      >
        {tooltip}
      </div>
    </div>
  )
}

export default Tooltip;
