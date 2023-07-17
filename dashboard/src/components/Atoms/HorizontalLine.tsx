import clsx from 'clsx';
import React from 'react';
export const HorizontalLine = ({
  thicknessClass = 'h-1', //set thickness using height classes
  colorClass = 'bg-white/50', //set color using background-color classes
  className,
}: {
  thicknessClass?: string;
  colorClass?: string;
  className?: string;
}) => {
  return (
    <div
      className={clsx(thicknessClass, colorClass, className)}
      style={{ width: '100%' }}
    />
  );
};
