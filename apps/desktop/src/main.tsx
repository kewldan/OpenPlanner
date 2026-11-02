import "@openplanner/ui/styles.css";
import {
  applyTheme,
  Editor,
  installProject,
  installSelftest,
  loadTheme,
} from "@openplanner/editor";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

applyTheme(loadTheme());
installProject();
void installSelftest();

const root = document.getElementById("root");
if (!root) throw new Error("В index.html нет #root");

createRoot(root).render(
  <StrictMode>
    <Editor />
  </StrictMode>,
);
