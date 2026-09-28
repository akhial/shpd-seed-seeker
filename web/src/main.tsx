import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./app/App";
import { initEditor } from "./engine/editor";

const root = createRoot(document.getElementById("root")!);

// The query builder draws its requirements through the lean editor module,
// so it loads before the first render; the search engine stays lazy.
initEditor().then(
  () =>
    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    ),
  (error: unknown) =>
    root.render(
      <p className="d1-boot-failure" role="alert">
        Seed Seeker could not load its requirement editor (
        {error instanceof Error ? error.message : String(error)}). Reload the page to try again.
      </p>,
    ),
);
