import {
  autoUpdate,
  flip,
  offset,
  Placement,
  shift,
  useFloating,
  arrow,
} from '@floating-ui/react-dom';
import { isAbsolute } from 'path';
import { useRef } from 'react';

export default function ToolTip({
  text,
  className,
  position = 'top',
  children,
}: {
  text: string;
  className?: string;
  position?: Placement;
  children: React.ReactNode;
}): JSX.Element {
  const arrowRef = useRef(null);
  const {
    x,
    y,
    reference,
    floating,
    placement,
    middlewareData: { arrow: { x: arrowX, y: arrowY } = {} },
  } = useFloating({
    placement: position,
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(4),
      flip(),
      shift({ padding: 5 }),
      arrow({ element: arrowRef }),
    ],
  });

  return (
    <>
      <div ref={reference} className="w-fit h-fit">{children}</div>
      <div
        ref={floating}
        style={{
          top: y ?? 0,
          left: x ?? 0,
        }}
        className={'text-gray-300 absolute bg-gray-800 p-1 rounded'}
      >
        {text}
        {/* TODO: wilbur pls make this look nicer */}
        <div
          ref={arrowRef}
          style={{
            top: arrowY ?? 0,
            left: arrowX ?? 0,
          }}
          className={'absolute bg-gray-800 w-3 h-3 rotate-45'}
        />
      </div>
    </>
  );
}
