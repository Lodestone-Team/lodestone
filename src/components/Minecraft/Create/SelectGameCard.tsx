import GameIcon from 'components/Atoms/GameIcon';
import React from 'react';
import clsx from 'clsx';
const SelectGameCard = ({
  title,
  description,
  className,
}: {
  title: string;
  description: string;
  className?: string;
}) => {
  return (
    <div className={clsx('', className)}>
      <span>
        {title}
        {/* <GameIcon
          game_type={'minecraft'}
          game_flavour={'vanilla'}
          className="h-4 w-4"
        /> */}
      </span>
    </div>
  );
};

export default SelectGameCard;
