import "./InstanceList.css";

import React, { useContext, useEffect, useState } from "react";

import Instance from "./Instance";
import InstanceCreator from "./InstanceCreator";
import { ServerContext } from "../contexts/ServerContext";

var utils = require("../utils")

async function getStatus(domain, port) {
  let response = await fetch(`https://${domain}:${port}/api/instances`);
  let instances = await response.json();
  return instances;
}

export default function InstanceList() {
  const [instances, setInstances] = useState([]);
  const { pollrate, domain, webport } = useContext(ServerContext);


  // useEffect(() => {
  //   getStatus(domain, webport).then(setInstances);
  // }, [domain, webport]);

  utils.useInterval(() => {
    getStatus(domain, webport).then(setInstances);
  }, pollrate, true);

  return (
    <div className="instance-list">
      {
        instances.map(instance => (
          <Instance key={instance.uuid} {...instance} />
        ))
      }
      <InstanceCreator />
    </div>
  );
};