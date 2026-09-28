import init, { requirement_board, requirement_editor } from "./editor-pkg/seedfinder_editor.js";
import type { InitInput } from "./editor-pkg/seedfinder_editor.js";
import type { BoardRequest, BoardResponse, EditorRequest, EditorResponse } from "./types";

// The requirement editor's rules live in the shared core
// (docs/requirement-editor.md). The web reaches them through this lean module,
// which carries no world generation, so it loads before the first render
// while the engine module stays lazy. Every envelope call goes through here.

/** An envelope's answer, or why there is none: its error document or a trap. */
export type EditorAnswer<T> = { ok: true; value: T } | { ok: false; error: string };

let editorPromise: Promise<void> | undefined;
let loaded = false;

/**
 * Instantiates the editor module once. The browser fetches it next to the
 * bundle; tests pass the module's bytes, since Node cannot fetch `file:` URLs.
 */
export function initEditor(source?: InitInput): Promise<void> {
  editorPromise ??= init({
    module_or_path: source ?? new URL("./editor-pkg/seedfinder_editor_bg.wasm", import.meta.url),
  }).then(() => {
    loaded = true;
  });
  return editorPromise;
}

/** Whether `initEditor` has finished, so the synchronous calls below may run. */
export const editorLoaded = (): boolean => loaded;

/**
 * Sends one request through an envelope. The envelopes answer every request,
 * but a bug in the core traps the module, and that must not take the page
 * down with it.
 */
function call<T>(envelope: (request: string) => string, request: unknown): EditorAnswer<T> {
  try {
    const answer = JSON.parse(envelope(JSON.stringify(request))) as T | { error: string };
    if (answer && typeof answer === "object" && "error" in answer)
      return { ok: false, error: answer.error };
    return { ok: true, value: answer };
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
}

/** The board after the request's edits, with everything it draws. */
export function requirementBoard(request: BoardRequest): EditorAnswer<BoardResponse> {
  return call(requirement_board, request);
}

/** The requirement sheet: a draft opened, changed or saved. */
export function requirementEditor(request: EditorRequest): EditorAnswer<EditorResponse> {
  return call(requirement_editor, request);
}
