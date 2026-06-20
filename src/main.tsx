import "modern-css-reset/dist/reset.min.css";
import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router";
import App from "./App/App";
import Play from "./pages/Play/Play";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<App />} >
          <Route index element={<Play />} />
        </Route>
      </Routes>
    </BrowserRouter>
  </React.StrictMode>,
);
