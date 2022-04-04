import "./Dashboard.scss";

import React, { useState } from "react";

import InstanceList from "../components/InstanceList";
import { ServerContext } from "../contexts/ServerContext";
import SystemMonitor from "../components/SystemMonitor";

const devServerContext = {
  pollrate: 5000,
  api_domain: `http://${location.hostname}:8000`,
  api_path: "/api/v1",
}

const prodServerContext = {
  pollrate: 2500,
  api_domain: "",
  api_path: "/api/v1",
}

export default function Dashboard() {
  // eslint-disable-next-line no-unused-vars
  const [serverContext, setServerContext] = useState((process.env.NODE_ENV !== "production") ? devServerContext : prodServerContext);

  return (
    <ServerContext.Provider value={serverContext}>
      <div className="dashboard">
        <h1 className="title">Dashboard</h1>
        <InstanceList />
        <SystemMonitor />
      </div>
    </ServerContext.Provider>
  )
}
