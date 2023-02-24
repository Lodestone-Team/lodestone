import { ReactNode } from 'react';
import * as React from 'react';

interface ItemProps {
  children: ReactNode;
  className?: string;
}

export function PlayerListItem({ children, className = '' }: ItemProps) {
  return (
    <div className={`flex items-center rounded-xl ${className}`}>
    {children}
    </div>
  );
}

interface CardProps {
  children: ReactNode;
  className?: string;
}

export function PlayerListCard({ children, className }: CardProps) {
  const numItems = React.Children.count(children);

  if (numItems === 0) {
    return null;
  }

  if (numItems === 1) {
    return (
      <div
        className={`flex h-fit w-full flex-col rounded border border-gray-faded/30 bg-gray-850 py-2 ${className}`}
      >
        {children}
      </div>
    );
  }

  return (
    <div
      className={`flex h-fit w-full flex-col rounded border border-gray-faded/30 bg-gray-850 py-2 ${className}`}
    >
      {React.Children.map(children, (child, index) => (
        <>
          {index > 0 && <hr className='my-2 border-gray-faded/30' />}
          {child}
        </>
      ))}
    </div>
  );
}
