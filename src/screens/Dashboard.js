import "./Dashboard.css";

import React, { useEffect, useState } from "react";

import InstanceList from "./InstanceList";
import SystemMonitor from "../components/SystemMonitor";

export default function Dashboard() {
  return (
    <div className="dashboard">
      <h1 className="title">Dashboard</h1>
      <InstanceList />
      <SystemMonitor/>
    </div>
  )
}
