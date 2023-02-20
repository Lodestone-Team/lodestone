import React, { Fragment, useContext, useState, useEffect } from 'react';
import Button from 'components/Atoms/Button';

const FileContextMenu = ({
  refProp,
  file,
  coords,
}) => {
  return (
    <div className="fixed right-0 z-50 mt-1.5 w-40 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-900 drop-shadow-md focus:outline-none"
      style={{top: coords.y + "px", left: coords.x + "px", position: "absolute" }}
      ref={refProp}
    >
      <div className="py-2">
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="Copy"
          align="end"
          variant="text"
          intention="primary"
        />
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="Cut"
          align="end"
          variant="text"
          intention="primary"
        />
      </div>
      <div className="py-2">
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="Rename"
          align="end"
          variant="text"
          intention="primary"
        />
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="Delete"
          align="end"
          variant="text"
          intention="primary"
        />
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="Unzip"
          align="end"
          variant="text"
          intention="primary"
        />
      </div>
      <div className="py-2">
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="New folder"
          align="end"
          variant="text"
          intention="primary"
        />
        <Button
          className="w-full whitespace-nowrap rounded-none bg-gray-900 px-2.5 text-small"
          label="New file"
          align="end"
          variant="text"
          intention="primary"
        />
      </div>
    </div>
  );
};

export default FileContextMenu;
