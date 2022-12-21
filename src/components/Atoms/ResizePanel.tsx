import React, { useEffect } from 'react';
import { DraggableCore } from 'react-draggable';
import { getHeight, getWidth } from 'utils/util';
import { DraggableEvent, DraggableData } from 'react-draggable';
import clsx from 'clsx';

const setCursor = (cursor: string) => {
  document.body.style.cursor = cursor;
};

/**
 * ResizePanel
 * A resizable panel that should be used inside a flex container
 * Uses a DraggableCore component to handle the resizing
 *
 * To use this as a controlled component, pass in the `size` prop and the `onResize` callback
 *
 */
export default function ResizePanel({
  direction,
  containerClassNames: containerClassNamesProps = '',
  contentClassNames: contentClassNamesProps = '',
  resizeBarClassNames: resizeBarClassNamesProps = '',
  children,
  style,
  size: initialSize,
  minSize: minSizeProps = 0,
  maxSize = Infinity,
  grow = false,
  validateSize: shouldValidateSize = true,
  onResize,
}: {
  direction: 'n' | 's' | 'e' | 'w';
  containerClassNames?: string;
  contentClassNames?: string;
  resizeBarClassNames?: string;
  children: React.ReactNode;
  style?: React.CSSProperties;
  size: number;
  minSize?: number;
  maxSize?: number;
  grow?: boolean;
  validateSize?: boolean;
  onResize?: (size: number) => void;
}) {
  const contentRef = React.useRef<HTMLDivElement>(null);
  const wrapperRef = React.useRef<HTMLDivElement>(null);

  const isHorizontal = direction === 'w' || direction === 'e';
  const [size, setSizeState] = React.useState(0);
  const [targetSize, setTargetSize] = React.useState(0); //the size the user is dragging to, usually not valid

  const setSize = (size: number) => {
    if (onResize) onResize(size);
    setSizeState(size);
  };

  const validateSize = () => {
    if (!shouldValidateSize) return;
    if (grow) return;
    const content = contentRef.current;
    const wrapper = wrapperRef.current;
    if (!content || !wrapper) return;
    const actualContent = content.children[0] as HTMLElement;
    const containerParent = wrapper.parentElement as HTMLElement;

    // Or if our size doesn't equal the actual content size, then we
    // must have pushed past the min size of the content, so resize back
    //let minSize = isHorizontal ? $(actualContent).outerWidth(true) : $(actualContent).outerHeight(true);

    let minSize = isHorizontal
      ? actualContent.scrollWidth
      : actualContent.scrollHeight;

    const margins = isHorizontal
      ? getWidth(actualContent, 'full') - getWidth(actualContent, 'outer')
      : getHeight(actualContent, 'full') - getHeight(actualContent, 'outer');

    minSize += margins;

    if (size !== minSize) {
      setSize(minSize);
      setTargetSize(minSize);
    } else {
      // If our resizing has left the parent container's content overflowing
      // then we need to shrink back down to fit
      const overflow = isHorizontal
        ? containerParent.scrollWidth - containerParent.clientWidth
        : containerParent.scrollHeight - containerParent.clientHeight;

      if (overflow) {
        console.log('overflow', overflow);
        const newSize = isHorizontal
          ? actualContent.clientWidth - overflow
          : actualContent.clientHeight - overflow;
        setSize(newSize);
        setTargetSize(newSize);
      }
    }
  };

  useEffect(() => {
    const content = contentRef.current;
    if (!content) return;

    setSize(initialSize);
    setTargetSize(initialSize);
    validateSize();
    // initialSize is intentionally left out of the dependency array
  }, [isHorizontal]);

  const handleDrag = (e: DraggableEvent, ui: DraggableData) => {
    const factor = direction === 'e' || direction === 's' ? -1 : 1;

    // modify the size based on the drag delta
    const delta = isHorizontal ? ui.deltaX : ui.deltaY;
    const newTargetSize = targetSize - delta * factor;
    const newSize = Math.min(Math.max(minSizeProps, newTargetSize), maxSize);
    setTargetSize(newTargetSize);
    setSize(newSize);
  };

  const handleDragEnd = (e: DraggableEvent, ui: DraggableData) => {
    setCursor('auto');
    validateSize();
    setTargetSize(size);
  };

  const containerClassNames = `flex items-stretch flex-nowrap ${
    isHorizontal ? 'flex-row' : 'flex-col'
  } ${containerClassNamesProps}`;

  // eslint-disable-next-line prefer-const
  let containerStyle = { ...style } || ({} as React.CSSProperties);
  if (size !== 0 && !grow) {
    containerStyle.flexGrow = 0;
    containerStyle[isHorizontal ? 'width' : 'height'] = 'auto';
  }

  const resizeBarClassNames = `bg-clip-content z-10 bg-gray-faded/30 ${
    resizeBarClassNamesProps
      ? resizeBarClassNamesProps
      : isHorizontal
      ? 'cursor-ew-resize pl-1.5 pr-1.5 -ml-1.5 -mr-1.5'
      : 'cursor-ns-resize pt-1.5 pb-1.5 -mt-1.5 -mb-1.5'
  }`;

  const contentStyle = isHorizontal
    ? { width: size + 'px' }
    : { height: size + 'px' };

  const content = [
    <div
      key="content"
      ref={contentRef}
      className={clsx(
        "flex grow self-stretch",
        isHorizontal ? "flex-row" : "flex-col",
        contentClassNamesProps,
      )}
      style={grow ? {} : contentStyle}
    >
      {children}
    </div>,
  ];

  const handle = (
    <DraggableCore
      key="handle"
      onDrag={handleDrag}
      onStop={handleDragEnd}
      onStart={() => setCursor(isHorizontal ? 'ew-resize' : 'ns-resize')}
    >
      <div className={resizeBarClassNames}></div>
    </DraggableCore>
  );

  return (
    <div
      ref={wrapperRef}
      className={containerClassNames}
      style={containerStyle}
    >
      {(direction === 'w' || direction === 'n') && !grow ? handle : null}
      {content}
      {(direction === 'e' || direction === 's') && !grow ? handle : null}
    </div>
  );
}
