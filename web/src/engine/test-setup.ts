import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { initEditor } from "./editor";

// The app loads the requirement editor before its first render; every test
// file gets it the same way. Node cannot fetch the module, so it is
// instantiated from its bytes. (`new URL(…, import.meta.url)` would name the
// dev server's copy under happy-dom, where Vite rewrites asset URLs.)
await initEditor(await readFile(join(import.meta.dirname, "editor-pkg/seedfinder_editor_bg.wasm")));
