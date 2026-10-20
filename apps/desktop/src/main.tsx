/**
 * Entry point. Theme is applied synchronously before the first React render
 * (docs/02 §2: the persisted theme resolves before the window is shown).
 */
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import { initTheme } from "./theme";
import "./styles/global.css";

initTheme();

createRoot(document.getElementById("root")!).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>,
);
