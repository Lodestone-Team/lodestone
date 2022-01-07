import "./InstanceList.css";

import React, { useContext, useState } from "react";

import Instance from "./Instance";
import InstanceCreator from "./InstanceCreator";
import { ServerContext } from "../contexts/ServerContext";

var utils = require("../utils")

async function getStatus(api_domain, api_path) {
  let response = await fetch(`${api_domain}${api_path}/instances`);
  let instances = await response.json();
  return instances;
}

export default function InstanceList() {
  const [instances, setInstances] = useState([]);
  const { pollrate, api_domain, api_path } = useContext(ServerContext);

  utils.useInterval(() => {
    getStatus(api_domain, api_path).then(setInstances);
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