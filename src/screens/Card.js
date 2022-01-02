import "./Card.css";

import React, { useEffect, useRef } from "react";

import VanillaTilt from "vanilla-tilt";

export default function Dashboard({children, className, options}) {
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
