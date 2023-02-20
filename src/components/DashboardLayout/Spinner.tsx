import React from 'react';

const Spinner = () => {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-gray-900 bg-opacity-75">
      <div className="relative h-24 w-24">
        <div
          className="absolute top-0 left-0 h-full w-full animate-spin rounded-full border-4 border-t-4 border-blue-400"
          style={{
            borderBottomColor: 'transparent',
          }}
        ></div>
      </div>
    </div>
  );
};

export default Spinner;
