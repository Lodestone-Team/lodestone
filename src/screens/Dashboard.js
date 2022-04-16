import "./Dashboard.scss";

import React, { useState, useEffect } from "react";

import InstanceList from "../components/InstanceList";
import { ServerContext } from "../contexts/ServerContext";
import SystemMonitor from "../components/SystemMonitor";

const defaultServerContext = {
  pollrate: 5000,
  api_domain: "127.0.0.1:8000",
  api_path: "/api/v1",
}

export default function Dashboard() {
  //grab serverContext from localStorage or set default
  const [serverContext, setServerContext] = useState(
    JSON.parse(localStorage.getItem("serverContext")) || defaultServerContext
  );

  //update serverContext in localStorage everytime it changes using useEffect hook
  useEffect(() => {
    localStorage.setItem("serverContext", JSON.stringify(serverContext));
  }, [serverContext]);


  return (
    <div className="dashboard-wrapper">
      <ServerContext.Provider value={serverContext}>
        <div className="dashboard">
          <h1 className="title">Dashboard</h1>
          <InstanceList />
          <SystemMonitor />
        </div>
      </ServerContext.Provider>
      <div className="sidebar">
        {/* crude server context setting */}
        <div className="setting">
          <h3>Temporary Settings</h3>
          <div className="setting-input">
            <label htmlFor="api_domain">Client Address</label>
            <input
              type="text"
              name="api_domain"
              value={serverContext.api_domain}
              onChange={(e) =>
                setServerContext({
                  pollrate: serverContext.pollrate,
                  api_domain: e.target.value,
                  api_path: serverContext.api_path,
                })
              }
            />
          </div>
        </div>
      </div>
    </div>
  )
}
