import React, { useEffect } from 'react';
import { DraggableCore } from 'react-draggable';
import { getHeight, getWidth } from 'utils/util';
import { DraggableEvent, DraggableData } from 'react-draggable';

const setCursor = (cursor: string) => {
  document.body.style.cursor = cursor;
};

export default function ResizePanel({
  direction,
  containerClassNames: containerClassNamesProps = '',
  resizeBarClassNames: resizeBarClassNamesProps = '',
  children,
  style,
  size: initialSize,
  minSize: minSizeProps = 0,
  maxSize = Infinity,
  validateSize: shouldValidateSize = true,
  onResize,
}: {
  direction: 'n' | 's' | 'e' | 'w';
  containerClassNames?: string;
  resizeBarClassNames?: string;
  children: React.ReactNode;
  style?: React.CSSProperties;
  size: number;
  minSize?: number;
  maxSize?: number;
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
    // const actualContent = content.children[0] as HTMLElement;
    // const initialSize = isHorizontal
    //   ? getWidth(actualContent, 'full')
    //   : getHeight(actualContent, 'full');

    // Initialize the size value based on the content's current size

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
  if (size !== 0) {
    containerStyle.flexGrow = 0;
    containerStyle[isHorizontal ? 'width' : 'height'] = 'auto';
  }

  const resizeBarClassNames = `flex items-center justify-center z-10 bg-transparent ${
    isHorizontal
      ? 'cursor-ew-resize w-2 -ml-1 -mr-1'
      : 'cursor-ns-resize h-2 -mt-1 -mb-1'
  } ${resizeBarClassNamesProps}`;

  const contentStyle = isHorizontal
    ? { width: size + 'px' }
    : { height: size + 'px' };

  const contentClassName = `flex grow self-stretch ${
    isHorizontal ? 'flex-row' : 'flex-col'
  }`;

  const content = [
    <div
      key="content"
      ref={contentRef}
      className={contentClassName}
      style={contentStyle}
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
      {direction === 'w' || direction === 'n' ? handle : null}
      {content}
      {direction === 'e' || direction === 's' ? handle : null}
    </div>
  );
}

// class ResizePanel extends React.Component {
//   // constructor(props) {
//   //   super(props);
//   //   this.state = { size: 0 };

//   //   this.contentRef = React.createRef();
//   //   this.wrapperRef = React.createRef();
//   //   this.validateSize = debounce(this.validateSize, 100).bind(this);
//   // }

//   render() {}
// }

// export default ResizePanel;
