import {
  autoUpdate,
  flip,
  offset,
  Placement,
  shift,
  arrow,
} from '@floating-ui/react-dom';
import {
  useFloating,
  useHover,
  useInteractions,
} from '@floating-ui/react-dom-interactions';
import { useRef, useState } from 'react';

export default function ToolTip({
  text,
  position = 'bottom',
  distance = 4,
  children,
}: {
  text: string;
  position?: Placement;
  distance?: number;
  children: React.ReactNode;
}): JSX.Element {
  const arrowRef = useRef(null);
  const [open, setOpen] = useState(false);
  const {
    context,
    x,
    y,
    reference,
    floating,
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    placement,
    middlewareData: { arrow: { x: arrowX, y: arrowY } = {} },
  } = useFloating({
    open: open,
    onOpenChange: setOpen,
    placement: position,
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(distance),
      flip(),
      shift({ padding: 5 }),
      arrow({ element: arrowRef }),
    ],
  });
  const { getReferenceProps, getFloatingProps } = useInteractions([
    useHover(context),
  ]);

  return (
    <>
      <div ref={reference} className="h-fit w-fit" {...getReferenceProps()}>
        {children}
      </div>
      {open && (
        <div
          ref={floating}
          style={{
            top: y ?? 0,
            left: x ?? 0,
          }}
          className={
            'absolute select-none rounded bg-gray-500 p-1 text-small leading-none text-gray-300'
          }
          {...getFloatingProps()}
        >
          {text}
          {/* TODO: wilbur pls make this look nicer */}
          <div
            ref={arrowRef}
            style={{
              top: arrowY ?? 0,
              left: arrowX ?? 0,
            }}
            className={'absolute h-3 w-3 rotate-45 bg-gray-500'}
          />
        </div>
      )}
    </>
  );
}
