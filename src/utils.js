import React, { useEffect, useRef, useState } from 'react';

export const capitalize = (str) => {
  return str.charAt(0).toUpperCase() + str.slice(1);
}


export const truncateString = (str, num) => {
  if (str.length <= num) {
    return str
  }
  return str.slice(0, num) + '...'
}

// utils.js
export const useInterval = (callback, delay, tickAtStart = false) => {

  const savedCallback = useRef();

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);


  useEffect(() => {
    function tick() {
      savedCallback.current();
    }
    if (delay !== null) {
      if (tickAtStart) tick();

      const id = setInterval(tick, delay);
      return () => clearInterval(id);
    }
  }, [delay, tickAtStart]);
}