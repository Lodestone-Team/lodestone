import "./InstanceList.css";

import React, { useEffect, useState } from "react";

import Instance from "./Instance";
import PlusIcon from "../assets/plus.svg";

// const placeholder = [
//   {
//     name: "Survival",
//     version: "1.18.1",
//     flavour: "vanilla",
//     port: "25565",
//     uuid: "1234789123789127398127389",
//   },
//   {
//     name: "Minigames",
//     version: "1.8.9",
//     flavour: "forge",
//     port: "25567",
//     uuid: "12323123123",
//   },
//   {
//     name: "Testing",
//     version: "1.18",
//     flavour: "fabric",
//     port: "25568",
//     uuid: "21341414214124214412",
//   }
// ]

const domain = "127.0.0.1";
//TODO: replace this with proper domain name

export default function InstanceList() {
  const [instances, setInstances] = useState([]);

  React.useEffect(() => {
    fetch(`https://${domain}:8000/api/instances`)
      .then(response => response.json())
      .then(data => setInstances(data));
  }, []);

  return (
    <div className="instance-list">
      {
        instances.map(instance => (
          <Instance key={instance.uuid} {...instance} />
        ))
      }
      <img src={PlusIcon} alt="Plus Icon" className="new-instance-button"/>
    </div>
  );
};