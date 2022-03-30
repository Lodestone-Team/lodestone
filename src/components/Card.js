import "./Card.scss";

import React, { useEffect, useRef } from "react";

import VanillaTilt from "vanilla-tilt";

const defaultTilt = {
  scale: 1.05,
  speed: 1000,
  max: 10
};

export default function Card({ children, className, options = defaultTilt, tilt = false }) {
  const tiltRef = useRef(null);

  useEffect(() => {
    if (tilt)
      VanillaTilt.init(tiltRef.current, options);
  }, [options, tilt]);

  return (
    <div
      ref={tiltRef}
      className={"card " + (className ? className : "")}>
      {children}
    </div>
  )
}
