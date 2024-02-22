import { ReactNode } from 'react';
import * as React from 'react';

interface ItemProps {
  children: ReactNode;
  className?: string;
}

export function TunnelListItem({ children, className = '' }: ItemProps) {
  return (
    <div className={`rounded-xl ${className}`}>
    {children}
    </div>
  );
}

interface CardProps {
  children: ReactNode;
  className?: string;
}

export function TunnelListCard({ children, className }: CardProps) {
  return (
    <div
      className={`flex h-fit w-full flex-col rounded border border-gray-faded/30 bg-gray-850 p-2 ${className}`}
    >
      {React.Children.map(children, (child, index) => (
        <>
          {index > 0 && <hr className="my-2 mx-[-0.5rem] border-gray-faded/30" />}
          {child}
        </>
      ))}
    </div>
  );
}
