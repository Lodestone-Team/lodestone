import "./Dashboard.css";

import React, { useEffect, useState } from "react";

import InstanceList from "../components/InstanceList";
import {ServerContext} from "../contexts/ServerContext";
import SystemMonitor from "../components/SystemMonitor";

const defaultServerContext = {
  pollrate: 2500,
  domain: "127.0.0.1",
  webport: "8000"
}

export default function Dashboard() {
  return (
    <ServerContext.Provider value={defaultServerContext}>
      <div className="dashboard">
        <h1 className="title">Dashboard</h1>
        <InstanceList />
        <SystemMonitor />
      </div>
    </ServerContext.Provider>
  )
}
