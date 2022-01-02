import "./Card.css";

import React, { useEffect, useRef } from "react";

import VanillaTilt from "vanilla-tilt";

const defaultTilt = {
  scale: 1.025,
  speed: 1000,
  max: 8
};

export default function Dashboard({children, className, options = defaultTilt}) {
  // const tilt = useRef(null);

  // useEffect(() => {
  //   VanillaTilt.init(tilt.current, options);
  // }, [options]);
  
  return (
    <div className={"card " + className}>
      {children}
    </div>
  )
}
