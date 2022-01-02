import "./Dashboard.css";

import React, { useEffect, useState } from "react";

import InstanceList from "./InstanceList";

export default function Dashboard() {
  return (
    <div className="dashboard">
      <h1 className="title">Dashboard</h1>
      <InstanceList />
    </div>
  )
}
