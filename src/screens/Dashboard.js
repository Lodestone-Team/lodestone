import "./Dashboard.scss";

import React, { useState, useEffect } from "react";

import InstanceList from "../components/InstanceList";
import { ClientContext } from "../contexts/ClientContext";
import SystemMonitor from "../components/SystemMonitor";

const defaultClientContext = {
  pollrate: 5000,
  api_domain: "http://127.0.0.1:8000",
  api_path: "/api/v1",
}

export default function Dashboard() {
  //grab ClientContext from localStorage or set default
  const [clientContext, setClientContext] = useState(
    JSON.parse(localStorage.getItem("ClientContext")) || defaultClientContext
  );

  //update ClientContext in localStorage everytime it changes using useEffect hook
  useEffect(() => {
    localStorage.setItem("ClientContext", JSON.stringify(clientContext));
  }, [clientContext]);


  return (
    <div className="dashboard-wrapper">
      <ClientContext.Provider value={clientContext}>
        <div className="dashboard">
          <h1 className="title">Dashboard</h1>
          <InstanceList />
          <SystemMonitor />
        </div>
      </ClientContext.Provider>
      <div className="sidebar">
        {/* crude server context setting */}
        <div className="setting">
          <h3>Temporary Settings</h3>
          <div className="setting-input">
            <label htmlFor="api_domain">Client Address</label>
            <input
              type="text"
              name="api_domain"
              value={clientContext.api_domain}
              onChange={(e) =>
                setClientContext({
                  pollrate: clientContext.pollrate,
                  api_domain: e.target.value,
                  api_path: clientContext.api_path,
                })
              }
            />
          </div>
        </div>
      </div>
    </div>
  )
}
