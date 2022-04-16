import "./Dashboard.scss";

import React, { useState, useEffect } from "react";

import InstanceList from "../components/InstanceList";
import { ClientContext } from "../contexts/ClientContext";
import SystemMonitor from "../components/SystemMonitor";
import Login from "../components/Login";
import Register from "../components/Register";
import { auth, db, logout } from "../firebase";
import { query, collection, getDocs, where } from "firebase/firestore";
import { useAuthState } from "react-firebase-hooks/auth";
import Reset from "../components/Reset";

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


  /* Firebase authentication */
  const [user, loading] = useAuthState(auth);
  const [name, setName] = useState("");
  
  useEffect(() => {
    const fetchUserName = async () => {
      try {
        const q = query(collection(db, "users"), where("uid", "==", user?.uid));
        const doc = await getDocs(q);
        const data = doc.docs[0].data();
        setName(data.name);
      } catch (err) {
        console.error(err);
        alert("An error occured while fetching user data");
      }
    };

    if (loading) return;
    if (!user) {
      setName("Not logged in");
    } else {
      fetchUserName();
    }
  }, [user, loading]);

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
        <br />
        <h5>Logged in as {name} {name?.email}</h5>
        <Login />
        <Register />
        <Reset />
        <button className="dashboard__btn" onClick={logout}>
          Logout
        </button>
      </div>
    </div>
  )
}
