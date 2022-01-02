import './App.css';

import { Route, Routes } from "react-router-dom";

import Dashboard from "./screens/Dashboard.js";

function App() {
  return (
    <Routes>
      <Route path="/" element={<Dashboard />} />
    </Routes>
  );
}

export default App;
