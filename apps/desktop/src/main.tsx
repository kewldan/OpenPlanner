import "@openplanner/ui/styles.css";
import { Editor } from "@openplanner/editor";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

const root = document.getElementById("root");
if (!root) throw new Error("В index.html нет #root");

createRoot(root).render(
  <StrictMode>
    <Editor />
  </StrictMode>,
);
