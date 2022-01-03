import "./Card.css";

import React, { useEffect, useRef } from "react";

import VanillaTilt from "vanilla-tilt";

const defaultTilt = {
  scale: 1.025,
  speed: 1000,
  max: 5
};

export default function Card({children, className, options = defaultTilt}) {
  const tilt = useRef(null);

  useEffect(() => {
    VanillaTilt.init(tilt.current, options);
  }, [options]);
  
  return (
    <div ref={tilt} className={"card " + className}>
      {children}
    </div>
  )
}
