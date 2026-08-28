import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Toaster } from "@/components/ui/sonner";
import "./index.css";
import App from "./App.tsx";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
    {/* Sonner pauses the timer while the pointer is over a toast. */}
    <Toaster position="bottom-center" closeButton duration={2500} />
  </StrictMode>,
);
