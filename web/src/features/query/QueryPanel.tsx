import { FarmingFloors } from "./FarmingFloors";
import { useEffect, useState } from "react";
import { useStore } from "@tanstack/react-store";
import { LEVEL_GEN_CHALLENGES, challenges as challengeOptions } from "../../shared/game/catalog";
import { probabilityLabel } from "../../shared/format";
import {
  CheckIcon,
  CommandIcon,
  InfoIcon,
  LinkIcon,
  ReturnIcon,
  XIcon,
} from "../../shared/ui/icons";
import { BLACKSMITH_LAST_FLOOR, FLOOR_LIMIT_OPTIONS, fromQueryJson, toQueryJson } from "./query";
import type { ValidationResult } from "./validation";
import { questVariantLabel } from "../../shared/game/quests";
import {
  builtInPresets,
  loadPresets,
  maxWorkers,
  queryStore,
  savePresets,
  setWorkerCount,
  workerCountStore,
} from "../../app/store";
import type { Preset } from "../../app/store";
import { encodeShareLink } from "../../engine/wasm";
import { WANDMAKER_QUESTS } from "../../engine/types";
import type {
  AnalysisResult,
  BoardEdit,
  ChallengeName,
  EditorChange,
  EditorSheet,
  QueryState,
  WandmakerQuest,
} from "../../engine/types";
import { RequirementBoard } from "./requirements/RequirementBoard";
import type { BoardEditReport } from "./requirements/RequirementBoard";
import { editBoard, requirementBoardOf } from "./requirements/board";
import { RequirementEditor } from "./requirements/RequirementEditor";
import { changeSheet, openSheet, saveSheet } from "./requirements/sheet";
import type { SheetTarget } from "./requirements/sheet";
import { SliderRow } from "../../shared/ui/primitives";

const patchQuery = (patch: Partial<QueryState>) =>
  queryStore.setState((state) => ({ ...state, ...patch }));
const cloneQuery = (query: QueryState): QueryState => fromQueryJson(toQueryJson(query));

interface EditorSession {
  sheet: EditorSheet;
  /** Why the core could not answer the sheet's last request. */
  notice?: string;
}

/**
 * Applies board edits to the current query, writing the requirements back
 * only when they changed.
 */
function applyBoardEdits(edits: BoardEdit[]): BoardEditReport {
  const state = queryStore.state;
  const answer = editBoard(state, edits);
  if (!answer.ok) return { notice: answer.error, rekeyed: [] };
  const { changed, requirements, refused, rekeyed } = answer.value;
  if (changed) queryStore.setState(() => ({ ...state, requirements }));
  return { notice: refused?.message ?? null, rekeyed };
}

export function QueryPanel({
  analysis,
  validation,
  running,
  engineReady,
  onToggleSearch,
  isMac,
  shareNotice,
  onDismissShareNotice,
}: {
  analysis: AnalysisResult | undefined;
  validation: ValidationResult;
  running: boolean;
  engineReady: boolean;
  onToggleSearch: () => void;
  isMac: boolean;
  shareNotice: string | undefined;
  onDismissShareNotice: () => void;
}) {
  const query = useStore(queryStore);
  const workerCount = useStore(workerCountStore);
  const workerCeiling = maxWorkers();
  const [userPresets, setUserPresets] = useState<Preset[]>(() => loadPresets());
  const [namingPreset, setNamingPreset] = useState(false);
  const [presetName, setPresetName] = useState("");
  const [editor, setEditor] = useState<EditorSession | null>(null);
  const [editorFailure, setEditorFailure] = useState<string | null>(null);
  const [saved, setSaved] = useState<{ key: number } | null>(null);
  const [linkCopied, setLinkCopied] = useState(false);
  const [blanketHelpOpen, setBlanketHelpOpen] = useState(false);

  // A save lands on a chip, possibly a new one at the end of a long board.
  useEffect(() => {
    if (saved)
      document.querySelector(`[data-chip="${saved.key}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [saved]);

  useEffect(() => {
    if (!blanketHelpOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setBlanketHelpOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [blanketHelpOpen]);

  const shareQuery = () => {
    void encodeShareLink(toQueryJson(query))
      .then((link) => navigator.clipboard.writeText(link))
      .then(() => {
        setLinkCopied(true);
        window.setTimeout(() => setLinkCopied(false), 1_200);
      })
      .catch(() => undefined);
  };

  const applyPreset = (preset: Preset) => {
    queryStore.setState(() => cloneQuery(preset.query));
  };

  const currentQueryJson = JSON.stringify(toQueryJson(query));
  const presetFingerprint = (preset: Preset) => JSON.stringify(toQueryJson(preset.query));
  const builtInMatch = builtInPresets.findIndex(
    (preset) => presetFingerprint(preset) === currentQueryJson,
  );
  const userMatch =
    builtInMatch >= 0
      ? -1
      : userPresets.findIndex((preset) => presetFingerprint(preset) === currentQueryJson);
  const selectedPreset =
    builtInMatch >= 0 ? `b:${builtInMatch}` : userMatch >= 0 ? `u:${userMatch}` : "";

  const saveCurrentPreset = () => {
    const name = presetName.trim();
    if (!name) return;
    const snapshot = cloneQuery(query);
    const next = [...userPresets];
    const existing = next.findIndex((preset) => preset.name.toLowerCase() === name.toLowerCase());
    if (existing >= 0) next[existing] = { name: next[existing].name, query: snapshot };
    else next.push({ name, query: snapshot });
    setUserPresets(next);
    savePresets(next);
    setNamingPreset(false);
    setPresetName("");
  };

  const deletePreset = (name: string) => {
    const next = userPresets.filter((preset) => preset.name !== name);
    setUserPresets(next);
    savePresets(next);
  };

  // The board of both sections, its header counts and the Start and Share
  // gate all read one answer of the shared core, drawn once per change of
  // the requirements. A failure surfaces in the query pane's error boundary.
  const drawn = requirementBoardOf(query);
  if (!drawn.ok) throw new Error(drawn.error);
  const { items, counts, resin: resinChip } = drawn.value;
  // The board draws every chip a sheet can open, so an open the core cannot
  // answer is a failure of the editor, which the boundary reports too.
  if (editorFailure) throw new Error(editorFailure);

  const openEditor = (target: SheetTarget) => {
    const answer = openSheet(queryStore.state, target);
    if (answer.ok) setEditor({ sheet: answer.value });
    else setEditorFailure(answer.error);
  };

  const changeEditor = (change: EditorChange) =>
    setEditor((session) => {
      if (!session) return session;
      const answer = changeSheet(session.sheet, change);
      return answer.ok ? { sheet: answer.value } : { ...session, notice: answer.error };
    });

  const saveEditor = (session: EditorSession) => {
    const state = queryStore.state;
    const answer = saveSheet(state, session.sheet);
    if (!answer.ok) {
      setEditor({ ...session, notice: answer.error });
      return;
    }
    if ("refused" in answer.value) {
      setEditor({ sheet: answer.value.refused });
      return;
    }
    const { changed, requirements, resin, focus } = answer.value.saved;
    // Saving a chip unchanged keeps the query as it was.
    if (changed || resin) queryStore.setState(() => ({ ...state, requirements, ...resin }));
    setEditor(null);
    if (focus !== null) setSaved({ key: focus });
  };

  const toggleChallenge = (name: ChallengeName) => {
    const active = query.challenges.includes(name);
    patchQuery({
      challenges: active
        ? query.challenges.filter((value) => value !== name)
        : [...query.challenges, name],
    });
  };

  const floorCount = query.floorRequirements?.length ?? 0;
  const slotTotal =
    counts.ordinary + counts.blanket + Number(Boolean(query.arcaneResin)) + floorCount;
  const challengeCount = query.challenges.length;
  const wandmakerCount = Number(Boolean(query.wandmakerQuest));
  const blacksmithCount = Number(query.requireBlacksmith) + Number(query.excludeBlacksmithRewards);
  const hasRequirements = slotTotal > 0;
  const impossible = Boolean(analysis?.valid && analysis.impossible);
  const startDisabled = !running && (!engineReady || !validation.valid || impossible);

  return (
    <>
      <div className="d1-pane-head">
        <span>Query</span>
        <span className="d1-pane-head-side">
          <span className="d1-pane-head-info">
            {hasRequirements ? `${slotTotal} requirement${slotTotal === 1 ? "" : "s"}` : ""}
          </span>
          <button
            type="button"
            className="d1-io-btn"
            title="Copy a shareable link to this search"
            aria-label="Copy a shareable link to this search"
            disabled={!engineReady || !validation.valid}
            onClick={shareQuery}
          >
            {linkCopied ? <CheckIcon size={13} /> : <LinkIcon size={13} />}
            {linkCopied ? "Copied" : "Share"}
          </button>
        </span>
      </div>
      <div className="d1-pane-body">
        {shareNotice && (
          <div className="d1-banner d1-banner-bleed" role="alert">
            <span className="d1-grow">This share link couldn't be loaded: {shareNotice}</span>
            <button
              type="button"
              className="d1-banner-dismiss"
              aria-label="Dismiss"
              title="Dismiss"
              onClick={onDismissShareNotice}
            >
              <XIcon size={14} />
            </button>
          </div>
        )}
        <section className="d1-section">
          <div className="d1-section-head">
            <h3>Presets</h3>
          </div>
          <div className="d1-preset-row">
            <select
              className="d1-select d1-grow"
              value={selectedPreset}
              aria-label="Load preset"
              onChange={(event) => {
                const value = event.currentTarget.value;
                if (!value) return;
                const [scope, indexText] = value.split(":");
                const index = Number(indexText);
                const preset = scope === "b" ? builtInPresets[index] : userPresets[index];
                if (preset) applyPreset(preset);
              }}
            >
              <option value="">Load preset…</option>
              <optgroup label="Included">
                {builtInPresets.map((preset, index) => (
                  <option key={preset.name} value={`b:${index}`}>
                    {preset.name}
                  </option>
                ))}
              </optgroup>
              {userPresets.length > 0 && (
                <optgroup label="Saved">
                  {userPresets.map((preset, index) => (
                    <option key={preset.name} value={`u:${index}`}>
                      {preset.name}
                    </option>
                  ))}
                </optgroup>
              )}
            </select>
            <button
              type="button"
              className="d1-btn"
              onClick={() => {
                setNamingPreset((value) => !value);
                setPresetName("");
              }}
            >
              Save…
            </button>
          </div>
          {namingPreset && (
            <div className="d1-preset-row">
              <input
                className="d1-input d1-grow"
                autoFocus
                placeholder="Preset name"
                value={presetName}
                aria-label="Preset name"
                onChange={(event) => setPresetName(event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") saveCurrentPreset();
                  if (event.key === "Escape") setNamingPreset(false);
                }}
              />
              <button
                type="button"
                className="d1-btn d1-btn-primary"
                disabled={!presetName.trim()}
                onClick={saveCurrentPreset}
              >
                Save
              </button>
            </div>
          )}
          {userPresets.length > 0 && (
            <ul className="d1-preset-chips">
              {userPresets.map((preset) => (
                <li key={preset.name}>
                  <button
                    type="button"
                    className="d1-chip-name"
                    title="Apply preset"
                    onClick={() => applyPreset(preset)}
                  >
                    {preset.name}
                  </button>
                  <button
                    type="button"
                    className="d1-chip-delete"
                    aria-label={`Delete preset ${preset.name}`}
                    title="Delete preset"
                    onClick={() => deletePreset(preset.name)}
                  >
                    <XIcon size={14} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        {[false, true].map((blanket) => {
          const count = blanket ? counts.blanket : counts.ordinary;
          const board = (
            <RequirementBoard
              items={items.filter((item) => item.blanket === blanket)}
              resin={
                !blanket && query.arcaneResin && resinChip
                  ? {
                      chip: resinChip,
                      onEdit: () => openEditor({ type: "resin" }),
                      onRemove: () =>
                        patchQuery({ arcaneResin: undefined, arcaneResinFilter: undefined }),
                    }
                  : undefined
              }
              onEdits={applyBoardEdits}
              onEdit={(key) => openEditor({ type: "row", key })}
              onAdd={() => openEditor({ type: "new", blanket })}
            />
          );
          return (
            <section
              className="d1-section"
              key={String(blanket)}
              aria-label={blanket ? "Blanket Requirements" : "Requirements"}
            >
              {blanket ? (
                <details className="d1-details d1-blanket-details">
                  <summary>
                    <span>Blanket Requirements</span>
                    {count > 0 && <span className="d1-count">{count}</span>}
                    <span
                      className="d1-blanket-help"
                      onMouseEnter={() => setBlanketHelpOpen(true)}
                      onMouseLeave={(event) => {
                        if (!event.currentTarget.contains(document.activeElement))
                          setBlanketHelpOpen(false);
                      }}
                      onFocus={() => setBlanketHelpOpen(true)}
                      onBlur={() => setBlanketHelpOpen(false)}
                      onClick={(event) => event.preventDefault()}
                    >
                      <button
                        type="button"
                        className="d1-blanket-help-button"
                        aria-label="About blanket requirements"
                        aria-describedby="blanket-requirements-help"
                        onClick={() => setBlanketHelpOpen(true)}
                      >
                        <InfoIcon size={16} />
                      </button>
                      <span
                        id="blanket-requirements-help"
                        role="tooltip"
                        className="d1-blanket-help-tooltip"
                        hidden={!blanketHelpOpen}
                      >
                        <span>
                          Add the items you need under Requirements, then add extra filters here.
                          Each blanket must match at least one item fulfilling your requirements
                          above, including wands contributing Arcane Resin. It does not require
                          another item.
                        </span>
                        <span>
                          Choose Any item in a category to let any of your required items in that
                          category satisfy the blanket. All filters in one blanket apply to the same
                          item; separate blankets can match the same or different items.
                        </span>
                        <span>
                          For example, require Lightning, Disintegration, and Frost wands at +2 or
                          higher. Add an Any wand blanket with exactly +3 and Wandmaker Reward as
                          its source to require one of those three wands to be the Wandmaker’s +3
                          reward.
                        </span>
                      </span>
                    </span>
                  </summary>
                  <div className="d1-details-body">{board}</div>
                </details>
              ) : (
                <>
                  <div className="d1-section-head">
                    <h3>Requirements</h3>
                  </div>
                  {board}
                </>
              )}
            </section>
          );
        })}

        <section className="d1-section">
          <div className="d1-section-head">
            <h3>Search scope</h3>
          </div>
          <SliderRow
            label="Floor limit"
            valueLabel={`first ${query.maxDepth} floor${query.maxDepth === 1 ? "" : "s"}`}
            values={FLOOR_LIMIT_OPTIONS}
            value={query.maxDepth}
            fill
            onChange={(value) => patchQuery({ maxDepth: value })}
          />
          <label className="d1-check d1-auto-trinket">
            <input
              type="checkbox"
              checked={query.autoApplyTrinket}
              disabled={query.requirements.some((r) => r.selectTrinket)}
              onChange={(event) => patchQuery({ autoApplyTrinket: event.target.checked })}
            />
            <span>AutoTrinket</span>
          </label>
          <p className="d1-caption">
            {query.requirements.some((r) => r.selectTrinket)
              ? "Uses your explicit trinket selection instead."
              : "Applies a helpful trinket at +3 at the first brewing opportunity. Keeps it only when the match needs it."}
          </p>
        </section>

        <section className="d1-section">
          <details className="d1-details">
            <summary>
              <span>Wandmaker</span>
              {wandmakerCount > 0 && <span className="d1-count">{wandmakerCount}</span>}
            </summary>
            <div className="d1-details-body">
              <label className="d1-field">
                <span className="d1-field-label">Quest</span>
                <span className="d1-field-control">
                  <select
                    className="d1-select"
                    value={query.wandmakerQuest ?? ""}
                    onChange={(event) =>
                      patchQuery({
                        wandmakerQuest: (event.currentTarget.value || undefined) as
                          | WandmakerQuest
                          | undefined,
                      })
                    }
                  >
                    <option value="">Any</option>
                    {WANDMAKER_QUESTS.map((variant) => (
                      <option key={variant} value={variant}>
                        {questVariantLabel(variant)}
                      </option>
                    ))}
                  </select>
                </span>
              </label>
            </div>
          </details>
        </section>

        <section className="d1-section">
          <details className="d1-details">
            <summary>
              <span>Blacksmith</span>
              {blacksmithCount > 0 && <span className="d1-count">{blacksmithCount}</span>}
            </summary>
            <div className="d1-details-body">
              <label
                className={`d1-check${query.maxDepth >= BLACKSMITH_LAST_FLOOR ? " d1-check-disabled" : ""}`}
              >
                <input
                  type="checkbox"
                  checked={query.requireBlacksmith}
                  disabled={query.maxDepth >= BLACKSMITH_LAST_FLOOR}
                  onChange={(event) =>
                    patchQuery({ requireBlacksmith: event.currentTarget.checked })
                  }
                />
                <span>Require accessible blacksmith</span>
              </label>
              <label className="d1-check">
                <input
                  type="checkbox"
                  checked={query.excludeBlacksmithRewards}
                  onChange={(event) =>
                    patchQuery({ excludeBlacksmithRewards: event.currentTarget.checked })
                  }
                />
                <span>Exclude Smith rewards</span>
              </label>
              <p className="d1-caption">
                Required items cannot come from the 2,000-favor Smith choice, leaving favor
                available for reforging.
              </p>
            </div>
          </details>
        </section>

        <section className="d1-section" aria-label="Rooms and feelings">
          <details className="d1-details">
            <summary>
              <span>Rooms and feelings</span>
              {floorCount > 0 && <span className="d1-count">{floorCount}</span>}
            </summary>
            <div className="d1-details-body">
              <FarmingFloors query={query} onChange={(next) => queryStore.setState(() => next)} />
            </div>
          </details>
        </section>

        {workerCeiling > 1 && (
          <section className="d1-section">
            <details className="d1-details">
              <summary>
                <span>Performance</span>
              </summary>
              <div className="d1-details-body">
                <SliderRow
                  label="Workers"
                  valueLabel={`${workerCount} of ${workerCeiling} cores`}
                  min={1}
                  max={workerCeiling}
                  value={Math.min(workerCount, workerCeiling)}
                  fill
                  onChange={setWorkerCount}
                />
                <p className="d1-caption">Number of search threads to spawn.</p>
              </div>
            </details>
          </section>
        )}

        <section className="d1-section">
          <details className="d1-details">
            <summary>
              <span>Challenges</span>
              {challengeCount > 0 && <span className="d1-count">{challengeCount}</span>}
            </summary>
            <p className="d1-caption">
              Searches simulate runs with the selected challenges enabled.
            </p>
            <div className="d1-challenge-list">
              {challengeOptions.map((challenge) => (
                <label className="d1-check" key={challenge.value}>
                  <input
                    type="checkbox"
                    checked={query.challenges.includes(challenge.value)}
                    onChange={() => toggleChallenge(challenge.value)}
                  />
                  <span>
                    {challenge.label}
                    <em>
                      {challenge.value === "badder_bosses"
                        ? "changes boss layouts"
                        : LEVEL_GEN_CHALLENGES.has(challenge.value)
                          ? "changes level generation"
                          : "no effect on seed content"}
                    </em>
                  </span>
                </label>
              ))}
            </div>
          </details>
        </section>
      </div>

      <div className="d1-query-foot">
        {hasRequirements && (
          // The status line always reserves one line so the foot height (and the scroll body) never changes
          // as the query toggles between possible and impossible. The warning is anchored to this line's
          // bottom and grows upward over the form, so its bottom edge sits one gap above the button — matching
          // the button's bottom spacing — instead of pushing the button down.
          <div className="d1-query-status">
            {impossible && (
              <div className="d1-impossible">
                <strong className="d1-impossible-title">Impossible query</strong>
                <p>{analysis?.valid ? analysis.notes[0] : undefined}</p>
              </div>
            )}
            {!validation.valid ? (
              <span className="d1-inline-error">{validation.errors[0]}</span>
            ) : (
              <span className="d1-analysis-line">
                {analysis?.valid && !analysis.impossible
                  ? probabilityLabel(analysis.probability)
                  : ""}
              </span>
            )}
          </div>
        )}
        <button
          type="button"
          className={`d1-btn d1-btn-big ${running ? "d1-btn-danger" : impossible ? "" : "d1-btn-primary"}`}
          disabled={startDisabled}
          onClick={onToggleSearch}
        >
          <span>{running ? "Cancel Search" : "Start Search"}</span>
          <kbd>
            {isMac ? <CommandIcon size={13} /> : <span className="d1-kbd-text">Ctrl</span>}
            <ReturnIcon size={13} />
          </kbd>
        </button>
      </div>

      {editor && (
        <RequirementEditor
          sheet={editor.sheet}
          notice={editor.notice}
          onChange={changeEditor}
          onSave={() => saveEditor(editor)}
          onCancel={() => setEditor(null)}
        />
      )}
    </>
  );
}
