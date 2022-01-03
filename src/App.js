import './App.css';
import 'react-toastify/dist/ReactToastify.css';

import { Route, Routes } from "react-router-dom";
import { Slide, ToastContainer } from 'react-toastify';

import Dashboard from "./screens/Dashboard.js";

function App() {
  return (
    <>
      <Routes>
        <Route path="/" element={<Dashboard />} />
      </Routes>
      <ToastContainer
        position="bottom-right"
        autoClose={5000}
        hideProgressBar={false}
        newestOnTop
        closeOnClick
        rtl={false}
        pauseOnFocusLoss
        draggable
        pauseOnHover
        transition={Slide}

      />
    </>
  );
}

export default App;
