import { Menu } from '@headlessui/react';
import Button from './Atoms/Button';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { IconDefinition } from '@fortawesome/free-solid-svg-icons';

import { faEllipsisVertical } from '@fortawesome/free-solid-svg-icons';

interface MenuItemProperty {
  className?: string;
  label: string;
  icon: IconDefinition;
  variant?: 'contained' | 'text';
  intention?: 'none' | 'info' | 'danger' | 'primary';
  disabled: boolean;
  onClick: (event: React.MouseEvent<HTMLButtonElement>) => void;
}

export interface MenuItemProps {
  menuItems: MenuItemProperty[];
}

export default function ButtonMenu({ menuItems }: MenuItemProps) {
  return (
    <Menu as="div" className="relative inline-block text-right">
      <Menu.Button
        as={FontAwesomeIcon}
        icon={faEllipsisVertical}
        className="h-4 w-4 select-none text-h2 text-white/50 hover:cursor-pointer hover:text-white/75"
      />
      <Menu.Items
        className="absolute right-0 z-10 mt-0.5 origin-top-left divide-y divide-gray-faded/30
          rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none"
      >
        <div className="py-2 px-1.5">
          {menuItems.map((menuItem, index) => (
            <Menu.Item key={index}>
              <Button
                className={menuItem.className}
                label={menuItem.label}
                iconRight={menuItem.icon}
                variant={menuItem.variant}
                intention={menuItem.intention}
                align="end"
                disabled={menuItem.disabled}
                onClick={menuItem.onClick}
              />
            </Menu.Item>
          ))}
        </div>
      </Menu.Items>
    </Menu>
  );
}
