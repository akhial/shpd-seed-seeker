import { useEffect, useMemo, useRef, useState } from "react";
import type {
  CSSProperties,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  ReactNode,
} from "react";
import { effectGlows } from "../../../shared/sprites/glow";
import type { Glow } from "../../../shared/sprites/glow";
import { requirementArt } from "../../../shared/sprites/requirement-art";
import { CheckIcon, PlusIcon, XIcon } from "../../../shared/ui/icons";
import { ARCANE_RESIN_SPRITE, itemArt } from "../../../shared/sprites/sprites";
import type {
  BoardEdit,
  BoardItemView,
  ChipBadges,
  ChipFace,
  ChipTag,
  ChipView,
  ResinChipView,
} from "../../../engine/types";
import { Sprite } from "../../../shared/ui/primitives";
import { rekey } from "./board";

/**
 * The requirement board: every requirement is a chip; drop one chip onto
 * another for an either/or cluster, drag a chip out of its cluster to make
 * it standalone again. Everything else is a property of the chip itself —
 * a lone chip or a cluster member alike: a stack badge (×N / ≤N) for "more
 * of the same kind", and a Σ badge for a stack whose items count their
 * levels towards one total. A cluster draws no badge of its own. Every drag
 * moves one item — for a stack, a bare copy of it — so the chip in flight
 * draws the core's `lifted` face without badges, and the dimmed chip it
 * leaves keeps its own face with the badges its stack keeps.
 *
 * The board draws what the shared core answers — its entries, their words,
 * which drops join and which are refused — and sends the gestures back as
 * edits. Board state names rows by key and entries by id, both stable across
 * the core's edits.
 */

const DRAG_THRESHOLD = 5;
const LONG_PRESS_MS = 480;
/** How long a refusal stays in the status line. */
const NOTICE_MS = 4_000;

/** The popover's mark for each relation line. */
const RELATION_GLYPHS: Record<ChipView["relations"][number]["glyph"], string> = {
  or: "or",
  sum: "Σ",
  times: "×",
};

function ChipSprite({ face, glows }: { face: ChipFace; glows?: Glow[] }) {
  return face.item ? (
    <Sprite art={requirementArt(face)} size={18} glow={glows} />
  ) : (
    <span className="d1-chip-wildcard" aria-hidden="true">
      <span className="d1-chip-wildcard-silhouette">
        <Sprite art={requirementArt(face)} size={18} />
      </span>
      <span className="d1-chip-wildcard-mark">?</span>
    </span>
  );
}

/**
 * A tag's look by its style. The resin a chip counts (`credit`) keeps the
 * plain tag's amber, as the web always drew the resin chip's amount and
 * `Mage +2`; its own class leaves room to tint it apart.
 */
const TAG_CLASS: Record<ChipTag["style"], string> = {
  plain: "d1-chip-tag",
  upgrade: "d1-chip-tag d1-chip-tag-up",
  credit: "d1-chip-tag d1-chip-tag-credit",
};

function Tags({ tags }: { tags: ChipTag[] }) {
  return tags.map((tag) => (
    <span key={tag.text} className={TAG_CLASS[tag.style]} title={tag.tooltip ?? undefined}>
      {tag.text}
    </span>
  ));
}

/** Blend evenly spaced effect colours around a stationary ring, as on macOS. */
function effectRingCss(glows: Glow[]): CSSProperties {
  const band = 360 / glows.length;
  const stops = glows.map((glow, index) => `${glow.color} ${index * band}deg`);
  stops.push(`${glows[0].color} 360deg`);
  return { "--d1-ring": `conic-gradient(${stops.join(", ")})` } as CSSProperties;
}

/**
 * What a chip shows of an item — sprite, name, tags, effect cue, trailing
 * tags and the uncursed check — without its badges, so the board's chip and
 * the drag ghost draw a face the same way.
 */
function ItemFace({ face }: { face: ChipFace }) {
  const effect = face.effect;
  const glows = effect && !effect.any_enchantment ? effectGlows(effect.effects) : [];
  const glow = glows[0] ?? null;
  return (
    <>
      <ChipSprite face={face} glows={glows} />
      <span className="d1-chip-name">{face.name}</span>
      <Tags tags={face.tags} />
      {/* Named items show a single effect through their sprite's glow.
        Wildcards keep their green question mark and show an effect badge. */}
      {effect &&
        (glows.length > 1 ? (
          <span
            className="d1-chip-effect d1-chip-effect-multi"
            style={effectRingCss(glows)}
            title={effect.label}
          >
            {glows.length}
          </span>
        ) : glow ? (
          face.item ? null : (
            <span
              className="d1-chip-effect"
              style={{ color: glow.color, backgroundColor: glow.color }}
              title={effect.label}
            />
          )
        ) : (
          <span
            className={`d1-chip-effect ${effect.any_enchantment ? "d1-chip-effect-any" : "d1-chip-effect-curse"}`}
            title={effect.label}
          />
        ))}
      <Tags tags={face.trailing_tags} />
      {face.uncursed && (
        <span className="d1-chip-tag d1-chip-tag-soft" title="Uncursed">
          <CheckIcon size={12} />
        </span>
      )}
    </>
  );
}

/**
 * The face of the item a move of `chip` carries: the bare copy the core lifts
 * from a stack, else the chip itself.
 */
const movingFace = (chip: ChipView): ChipFace => chip.lifted ?? chip;

type DropTarget =
  | { kind: "chip"; key: number }
  | { kind: "cluster"; group: number }
  | { kind: "delete" }
  | { kind: "board" };

/** What releasing a chip over a target does. */
type DropAction =
  | { type: "join"; target: number }
  | { type: "refuse"; message: string }
  | { type: "detach" }
  | { type: "remove_one" };

type DragSource = number | "resin";

interface DragState {
  source: DragSource;
  x: number;
  y: number;
  over: DropTarget | null;
}

interface MenuState {
  /** The chip the menu belongs to; the resin chip's menu only edits and removes. */
  key: DragSource;
  x: number;
  y: number;
}
interface PickState {
  source: number;
}
interface StepperState {
  id: string;
  which: "count" | "total";
}

/** What an edit did, as far as the board's own state cares. */
export interface BoardEditReport {
  /** A refusal or failure to say. */
  notice: string | null;
  /** Keys the core renumbered, `[old, new]`. */
  rekeyed: [number, number][];
}

export function RequirementBoard({
  items,
  onEdits,
  onEdit,
  onAdd,
  resin,
}: {
  /** This section's entries, as the core drew them. */
  items: BoardItemView[];
  onEdits: (edits: BoardEdit[]) => BoardEditReport;
  /** Opens the sheet on a chip, or answers why the core will not. */
  onEdit: (key: number) => string | null;
  onAdd: () => void;
  resin?: {
    chip: ResinChipView;
    onEdit: () => void;
    onRemove: () => void;
  };
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [pick, setPick] = useState<PickState | null>(null);
  const [stepper, setStepper] = useState<StepperState | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [hovered, setHoveredState] = useState<{ key: number; left: number; top: number } | null>(
    null,
  );
  const pressRef = useRef<{
    key: DragSource;
    x: number;
    y: number;
    timer: number | undefined;
    dragging: boolean;
  } | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const suppressResinClick = useRef(false);

  /** Every visible row by key, with the entry it belongs to. */
  const chips = useMemo(() => {
    const byKey = new Map<number, { chip: ChipView; item: BoardItemView }>();
    for (const item of items) for (const chip of item.chips) byKey.set(chip.key, { chip, item });
    return byKey;
  }, [items]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const edit = (...edits: BoardEdit[]) => {
    const report = onEdits(edits);
    setNotice(report.notice);
    if (report.rekeyed.length === 0) return;
    const { rekeyed } = report;
    setMenu(
      (current) =>
        current &&
        (current.key === "resin" ? current : { ...current, key: rekey(current.key, rekeyed) }),
    );
    setHoveredState((current) => current && { ...current, key: rekey(current.key, rekeyed) });
    setStepper(
      (current) =>
        current && {
          ...current,
          id: current.id.replace(/^r(\d+)$/, (_, key: string) => `r${rekey(Number(key), rekeyed)}`),
        },
    );
  };

  const hoveredKey = hovered?.key ?? null;
  const setHovered = (key: number | null, element?: HTMLElement) => {
    if (key === null || !element) {
      setHoveredState(null);
      return;
    }
    const rect = element.getBoundingClientRect();
    setHoveredState({
      key,
      left: Math.min(rect.left, window.innerWidth - 300),
      top: rect.bottom + 8,
    });
  };

  // ---- drag -----------------------------------------------------------------

  const targetAt = (x: number, y: number, source: DragSource): DropTarget | null => {
    const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop]");
    if (!element || !wrapRef.current?.contains(element)) return null;
    const kind = element.dataset.drop;
    // Resin can be removed by dragging, but never joins an either/or group.
    if (kind === "resin" || (source === "resin" && kind !== "delete")) return null;
    if (kind === "chip") return { kind: "chip", key: Number(element.dataset.chip) };
    if (kind === "cluster") return { kind: "cluster", group: Number(element.dataset.group) };
    if (kind === "delete") return { kind: "delete" };
    return { kind: "board" };
  };

  /** The drop the core allows: a join it lists, a refusal it words, or a detach. */
  const dropAction = (source: number, over: DropTarget): DropAction | null => {
    const chip = chips.get(source)?.chip;
    if (!chip) return null;
    // The remove target takes one item; the menu's Remove takes the whole stack.
    if (over.kind === "delete") return { type: "remove_one" };
    if (over.kind === "board") return chip.can_detach ? { type: "detach" } : null;
    const targets =
      over.kind === "chip"
        ? [over.key]
        : (items.find((item) => item.cluster === over.group)?.members ?? []);
    const target = targets.find((key) => chip.join.includes(key));
    if (target !== undefined) return { type: "join", target };
    const refusal = chip.refuse.find((entry) => targets.includes(entry.key));
    return refusal ? { type: "refuse", message: refusal.message } : null;
  };

  const updateDrag = (next: DragState | null) => {
    dragRef.current = next;
    setDrag(next);
  };

  const perform = (source: number, action: DropAction | null) => {
    if (!action) return;
    if (action.type === "refuse") setNotice(action.message);
    else if (action.type === "join") edit({ type: "join", source, target: action.target });
    else edit({ type: action.type, key: source });
  };

  const completeDrop = (state: DragState) => {
    const { source, over } = state;
    if (!over) return;
    if (source === "resin") {
      if (over.kind === "delete") resin?.onRemove();
      return;
    }
    perform(source, dropAction(source, over));
  };

  const onChipPointerDown = (key: DragSource) => (event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("[data-no-drag]")) return;
    if (key === "resin") suppressResinClick.current = false;
    event.currentTarget.setPointerCapture(event.pointerId);
    const timer =
      event.pointerType === "mouse"
        ? undefined
        : window.setTimeout(() => {
            const press = pressRef.current;
            if (!press || press.dragging) return;
            pressRef.current = null;
            // The resin chip is a button: the release must not also open its sheet.
            if (key === "resin") suppressResinClick.current = true;
            setMenu({ key, x: press.x, y: press.y });
          }, LONG_PRESS_MS);
    pressRef.current = { key, x: event.clientX, y: event.clientY, timer, dragging: false };
  };

  const onChipPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
    const press = pressRef.current;
    if (!press) return;
    if (!press.dragging) {
      if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_THRESHOLD) return;
      window.clearTimeout(press.timer);
      press.dragging = true;
      if (press.key === "resin") suppressResinClick.current = true;
      setMenu(null);
      setHovered(null);
      setPick(null);
      setStepper(null);
      setNotice(null);
    }
    updateDrag({
      source: press.key,
      x: event.clientX,
      y: event.clientY,
      over: targetAt(event.clientX, event.clientY, press.key),
    });
  };

  const editChip = (key: DragSource) => {
    if (key === "resin") resin?.onEdit();
    // The core refuses to open a row it cannot read, and says why.
    else if (chips.has(key)) setNotice(onEdit(key));
  };

  const removeChip = (key: DragSource) => {
    if (key === "resin") resin?.onRemove();
    else edit({ type: "remove", key });
  };

  /** Completes pick mode on `key`: the menu's and the keyboard's way to drop. */
  const pickChip = (source: number, key: number) => {
    setPick(null);
    if (key !== source) perform(source, dropAction(source, { kind: "chip", key }));
  };

  const onChipPointerUp = (event: ReactPointerEvent<HTMLElement>) => {
    const press = pressRef.current;
    pressRef.current = null;
    if (!press) return;
    window.clearTimeout(press.timer);
    if (press.dragging) {
      const state = dragRef.current;
      updateDrag(null);
      if (state)
        completeDrop({ ...state, over: targetAt(event.clientX, event.clientY, state.source) });
      return;
    }
    // The resin chip is a button and handles its native click, including keyboard activation.
    if (press.key === "resin") return;
    if (pick) pickChip(pick.source, press.key);
    else editChip(press.key);
  };

  const onChipPointerCancel = () => {
    const press = pressRef.current;
    pressRef.current = null;
    if (press) window.clearTimeout(press.timer);
    updateDrag(null);
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (dragRef.current) {
        pressRef.current = null;
        updateDrag(null);
      }
      setMenu(null);
      setPick(null);
      setStepper(null);
      setNotice(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onChipKeyDown = (key: DragSource) => (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      // The resin chip is a button; its native click opens its sheet.
      if (key === "resin") return;
      event.preventDefault();
      if (pick) pickChip(pick.source, key);
      else editChip(key);
    } else if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      removeChip(key);
    } else if (
      event.key === "ContextMenu" ||
      (event.shiftKey && event.key === "F10") ||
      event.key === "."
    ) {
      event.preventDefault();
      const rect = event.currentTarget.getBoundingClientRect();
      setMenu({ key, x: rect.left, y: rect.bottom });
    }
  };

  const onChipContextMenu = (key: DragSource) => (event: ReactMouseEvent<HTMLElement>) => {
    event.preventDefault();
    setMenu({ key, x: event.clientX, y: event.clientY });
  };

  // ---- rendering ---------------------------------------------------------------

  /** The hovered target's action while a chip is in flight. */
  const hoverAction =
    drag && drag.over && drag.source !== "resin" ? dropAction(drag.source, drag.over) : null;

  const dropClass = (target: DropTarget): string => {
    const over = drag?.over;
    if (!over || !hoverAction) return "";
    const same =
      over.kind === target.kind &&
      (over.kind === "chip"
        ? over.key === (target as { key: number }).key
        : over.kind === "cluster"
          ? over.group === (target as { group: number }).group
          : true);
    if (!same) return "";
    if (hoverAction.type === "detach") return " d1-drop-detach";
    if (hoverAction.type === "join") return " d1-drop-alternative";
    if (hoverAction.type === "refuse") return " d1-drop-refused";
    return "";
  };

  const renderChip = (chip: ChipView) => {
    const classes = ["d1-chip"];
    if (drag?.source === chip.key) classes.push("d1-chip-dragging");
    if (chip.problem) classes.push("d1-chip-error");
    // A chip picked up to join is the moving item in place: it draws, and is
    // named by, the item that moves.
    const picked = pick?.source === chip.key;
    const face = picked ? movingFace(chip) : chip;
    if (pick) {
      if (picked) classes.push("d1-chip-pick-source");
      else if (chips.get(pick.source)?.chip.join.includes(chip.key))
        classes.push("d1-chip-pickable");
    }
    return (
      <div
        key={chip.key}
        role="button"
        tabIndex={0}
        className={classes.join(" ") + dropClass({ kind: "chip", key: chip.key })}
        data-drop="chip"
        data-chip={chip.key}
        aria-label={face.description}
        onPointerDown={onChipPointerDown(chip.key)}
        onPointerMove={onChipPointerMove}
        onPointerUp={onChipPointerUp}
        onPointerCancel={onChipPointerCancel}
        onKeyDown={onChipKeyDown(chip.key)}
        onContextMenu={onChipContextMenu(chip.key)}
        onMouseEnter={(event) => setHovered(chip.key, event.currentTarget)}
        onMouseLeave={() => {
          if (hoveredKey === chip.key) setHovered(null);
        }}
        onFocus={(event) => setHovered(chip.key, event.currentTarget)}
        onBlur={() => {
          if (hoveredKey === chip.key) setHovered(null);
        }}
      >
        <ItemFace face={face} />
        {/* A chip picked up to join shows the one item that moves; a dragged
            chip's origin shows what its stack keeps while that item is away. */}
        {picked
          ? null
          : drag?.source === chip.key && chip.remaining_badges
            ? renderRemainingBadges(chip.remaining_badges)
            : renderBadges(chip)}
      </div>
    );
  };

  /** The badges a dragged chip's origin keeps: shown, never stepped. */
  const renderRemainingBadges = ({ count, total }: ChipBadges): ReactNode => (
    <>
      {count && (
        <span className="d1-stack-badge" title={count.tooltip}>
          {count.text}
        </span>
      )}
      {total && (
        <span className="d1-stack-badge" title={total.tooltip}>
          {total.text}
        </span>
      )}
    </>
  );

  /** A chip's stack (×N / ≤N) and combined-level (Σ) badges with their steppers. */
  const renderBadges = (chip: ChipView): ReactNode => {
    const { key, stack, badges } = chip;
    const id = `r${key}`;
    const editingCount = stepper?.id === id && stepper.which === "count";
    const editingTotal = stepper?.id === id && stepper.which === "total";
    const total = stack.total;
    return (
      <>
        {editingCount ? (
          <span
            className="d1-stack-edit"
            role="group"
            aria-label="How many"
            onPointerDown={(event) => event.stopPropagation()}
            data-no-drag
          >
            <button
              type="button"
              aria-label="One fewer"
              disabled={stack.count <= 1}
              onClick={() => edit({ type: "set_count", key, count: stack.count - 1 })}
            >
              −
            </button>
            <span className="d1-stack-badge">{stack.count_text}</span>
            <button
              type="button"
              aria-label="One more"
              disabled={stack.count >= stack.count_max}
              onClick={() => edit({ type: "set_count", key, count: stack.count + 1 })}
            >
              +
            </button>
            <button
              type="button"
              className="d1-stack-done"
              aria-label="Done"
              onClick={() => setStepper(null)}
            >
              ✓
            </button>
          </span>
        ) : (
          (badges.count || editingTotal) && (
            <button
              type="button"
              className="d1-stack-badge d1-stack-badge-btn"
              data-no-drag
              title={badges.count?.tooltip}
              onPointerDown={(event) => event.stopPropagation()}
              onClick={() => setStepper({ id, which: "count" })}
            >
              {badges.count?.text ?? stack.count_text}
            </button>
          )
        )}
        {editingTotal ? (
          <span
            className="d1-stack-edit d1-stack-edit-total"
            role="group"
            aria-label="Combined level"
            onPointerDown={(event) => event.stopPropagation()}
            data-no-drag
          >
            {/* Stepping below Σ ≥ 1 stops counting levels; stepping up from
                there starts again where the core starts it. */}
            <button
              type="button"
              aria-label="Lower total"
              disabled={total === null}
              onClick={() =>
                edit({ type: "set_total", key, total: total && total > 1 ? total - 1 : null })
              }
            >
              −
            </button>
            <span className="d1-stack-badge">{stack.total_text}</span>
            <button
              type="button"
              aria-label="Raise total"
              disabled={total !== null && total >= stack.level_capacity}
              onClick={() =>
                edit(
                  total === null
                    ? { type: "toggle_levels", key }
                    : { type: "set_total", key, total: total + 1 },
                )
              }
            >
              +
            </button>
            <button
              type="button"
              className="d1-stack-done"
              aria-label="Done"
              onClick={() => setStepper(null)}
            >
              ✓
            </button>
          </span>
        ) : (
          badges.total && (
            <button
              type="button"
              className="d1-stack-badge d1-stack-badge-btn"
              data-no-drag
              title={badges.total.tooltip}
              onPointerDown={(event) => event.stopPropagation()}
              onClick={() => setStepper({ id, which: "total" })}
            >
              {badges.total.text}
            </button>
          )
        )}
      </>
    );
  };

  const renderItem = (item: BoardItemView): ReactNode => {
    if (item.cluster === null) return renderChip(item.chips[0]);
    return (
      <div
        key={item.id}
        className={`d1-cluster${dropClass({ kind: "cluster", group: item.cluster })}`}
        data-drop="cluster"
        data-group={item.cluster}
        role="group"
        aria-label={item.label ?? undefined}
      >
        {item.chips.map((chip, position) => (
          <span key={chip.key} className="d1-cluster-member">
            {position > 0 && (
              <span className="d1-cluster-or" aria-hidden="true">
                or
              </span>
            )}
            {renderChip(chip)}
          </span>
        ))}
      </div>
    );
  };

  const dragSource = drag && drag.source !== "resin" ? chips.get(drag.source)?.chip : undefined;
  const draggingResin = drag?.source === "resin" && resin;
  const menuEntry = menu && menu.key !== "resin" ? chips.get(menu.key) : undefined;
  // The resin chip's menu has no requirement chip to draw from.
  const menuOnResin = menu?.key === "resin" && resin !== undefined;
  const hoveredChip = hovered ? chips.get(hovered.key)?.chip : undefined;
  const pickSource = pick ? chips.get(pick.source)?.chip : undefined;
  // The pick prompt names the item that moves, as its chip now draws it.
  const statusLine =
    hoverAction?.type === "refuse"
      ? hoverAction.message
      : pick
        ? `Either/or with… choose a chip${pickSource ? ` for ${movingFace(pickSource).title}` : ""}`
        : notice;

  return (
    <div ref={wrapRef} className="d1-board-wrap">
      <div
        className={`d1-board${drag ? " d1-board-dragging" : ""}${dropClass({ kind: "board" })}`}
        data-drop="board"
        onMouseDown={(event) => {
          // The steppers stop pointerdown, but this compatibility mousedown
          // still bubbles — closing the stepper here would unmount its
          // buttons before their click can fire.
          if ((event.target as HTMLElement).closest("[data-no-drag]")) return;
          setMenu(null);
          setStepper(null);
        }}
      >
        {items.map(renderItem)}
        {resin && (
          <button
            type="button"
            className={`d1-chip d1-resin-chip${draggingResin ? " d1-chip-dragging" : ""}`}
            data-drop="resin"
            aria-label="Edit Arcane Resin"
            title={resin.chip.tooltip ?? undefined}
            onPointerDown={onChipPointerDown("resin")}
            onPointerMove={onChipPointerMove}
            onPointerUp={onChipPointerUp}
            onPointerCancel={onChipPointerCancel}
            onClick={(event) => {
              const suppressed = suppressResinClick.current && event.detail > 0;
              suppressResinClick.current = false;
              if (suppressed) return;
              resin.onEdit();
            }}
            onKeyDown={onChipKeyDown("resin")}
            onContextMenu={onChipContextMenu("resin")}
          >
            <ResinChipBody chip={resin.chip} />
          </button>
        )}
        <button
          type="button"
          className="d1-chip d1-chip-add"
          onClick={onAdd}
          title="Add a requirement"
        >
          <PlusIcon size={13} />
          <span>Add</span>
        </button>
      </div>
      {drag && (
        <div
          className={`d1-delete-zone${drag.over?.kind === "delete" ? " d1-delete-zone-over" : ""}`}
          data-drop="delete"
        >
          <XIcon size={12} />
          <span>drop to remove</span>
        </div>
      )}
      {statusLine && (
        <div className="d1-board-status d1-mono" aria-live="polite">
          {statusLine}
        </div>
      )}
      {hovered && hoveredChip && !drag && !menu && (
        <ChipPopover
          chip={hoveredChip}
          moving={pick?.source === hoveredChip.key}
          style={{ left: hovered.left, top: hovered.top }}
        />
      )}
      {drag && (dragSource || draggingResin) && (
        <div
          className="d1-chip d1-chip-ghost"
          style={{ left: drag.x, top: drag.y }}
          aria-hidden="true"
        >
          {/* The one item that moves — a stack's bare copy, else the chip
              itself — never its badges. */}
          {dragSource ? (
            <ItemFace face={movingFace(dragSource)} />
          ) : (
            draggingResin && <ResinChipBody chip={resin.chip} amountOnly />
          )}
          {hoverAction?.type === "join" && (
            <span className="d1-chip-ghost-tag d1-ghost-alternative">or</span>
          )}
          {drag.over?.kind === "delete" && (
            <span className="d1-chip-ghost-tag d1-ghost-delete">remove</span>
          )}
        </div>
      )}
      {menu && (menuEntry || menuOnResin) && (
        <ChipMenu
          state={menu}
          chip={menuEntry?.chip ?? null}
          onClose={() => setMenu(null)}
          onEdit={() => {
            setMenu(null);
            editChip(menu.key);
          }}
          onPick={(key) => {
            setMenu(null);
            setPick({ source: key });
          }}
          onCount={(key, count) => edit({ type: "set_count", key, count })}
          onTotal={(key) => {
            setMenu(null);
            edit({ type: "toggle_levels", key });
          }}
          onDetach={(key) => {
            setMenu(null);
            edit({ type: "detach", key });
          }}
          onRemove={() => {
            setMenu(null);
            removeChip(menu.key);
          }}
        />
      )}
    </div>
  );
}

/**
 * The resin chip's face; the drag ghost shows only its amount, which the core
 * always puts first.
 */
function ResinChipBody({ chip, amountOnly }: { chip: ResinChipView; amountOnly?: boolean }) {
  return (
    <>
      <Sprite art={itemArt(ARCANE_RESIN_SPRITE)} size={18} />
      <span className="d1-chip-name">{chip.name}</span>
      <Tags tags={amountOnly ? chip.tags.slice(0, 1) : chip.tags} />
      {!amountOnly && chip.uncursed && (
        <span className="d1-chip-tag d1-chip-tag-soft" title="Uncursed wands">
          <CheckIcon size={12} />
        </span>
      )}
    </>
  );
}

/**
 * The detail card under a hovered or focused chip. A chip picked up to join
 * (`moving`) describes the item that moves, which carries none of its stack.
 */
function ChipPopover({
  chip,
  moving,
  style,
}: {
  chip: ChipView;
  moving: boolean;
  style: CSSProperties;
}) {
  const face = moving ? movingFace(chip) : chip;
  // A bare copy has no stack of its own to relate, nor the row's problem.
  const own = face === chip;
  return (
    <div className="d1-chip-pop" role="tooltip" style={style}>
      <div className="d1-chip-pop-title">{face.title}</div>
      {face.details.length > 0 && <div className="d1-chip-pop-sub">{face.details.join(" · ")}</div>}
      {own &&
        chip.relations.map((relation) => (
          <div key={relation.glyph} className="d1-chip-pop-rel">
            <span className="d1-chip-pop-glyph">{RELATION_GLYPHS[relation.glyph]}</span>
            <span>{relation.text}</span>
          </div>
        ))}
      {own && chip.problem && <div className="d1-chip-pop-error">{chip.problem}</div>}
    </div>
  );
}

/**
 * The chip's context menu: the gestures as words, for keyboard and touch. The
 * resin chip (`chip` null) is edited and removed like any chip, but stacks,
 * joins and detaches nothing.
 */
function ChipMenu({
  state,
  chip,
  onClose,
  onEdit,
  onPick,
  onCount,
  onTotal,
  onDetach,
  onRemove,
}: {
  state: MenuState;
  chip: ChipView | null;
  onClose: () => void;
  onEdit: () => void;
  onPick: (key: number) => void;
  onCount: (key: number, count: number) => void;
  onTotal: (key: number) => void;
  onDetach: (key: number) => void;
  onRemove: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const onDown = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [onClose]);
  const left = Math.min(state.x, window.innerWidth - 230);
  const top = Math.min(state.y, window.innerHeight - 260);
  const stack = chip?.stack;
  return (
    <div ref={ref} className="d1-chip-menu" role="menu" style={{ left, top }}>
      {/* A chip without a kind is a row the core cannot read, which only Remove applies to. */}
      {chip?.kind !== null && (
        <button type="button" role="menuitem" onClick={onEdit}>
          Edit…
        </button>
      )}
      {chip && chip.join.length > 0 && (
        <button type="button" role="menuitem" onClick={() => onPick(chip.key)}>
          <b>or</b>Either/or with…
        </button>
      )}
      {chip && stack?.can_change_count && (
        <>
          <span className="d1-chip-menu-rule" />
          <div className="d1-chip-menu-stepper" role="group" aria-label="How many">
            <span>
              <b>×</b>How many
            </span>
            <span className="d1-chip-menu-count">
              <button
                type="button"
                aria-label="One fewer"
                disabled={stack.count <= 1}
                onClick={() => onCount(chip.key, stack.count - 1)}
              >
                −
              </button>
              <span className="d1-mono">{stack.count}</span>
              <button
                type="button"
                aria-label="One more"
                disabled={stack.count >= stack.count_max}
                onClick={() => onCount(chip.key, stack.count + 1)}
              >
                +
              </button>
            </span>
          </div>
        </>
      )}
      {chip && stack?.can_count_levels && (
        <button type="button" role="menuitem" onClick={() => onTotal(chip.key)}>
          <b>Σ</b>
          {stack.total === null ? "Count levels together" : "Stop counting levels"}
        </button>
      )}
      {chip?.can_detach && (
        <>
          <span className="d1-chip-menu-rule" />
          <button type="button" role="menuitem" onClick={() => onDetach(chip.key)}>
            On its own
          </button>
        </>
      )}
      <span className="d1-chip-menu-rule" />
      <button type="button" role="menuitem" className="d1-chip-menu-danger" onClick={onRemove}>
        Remove
      </button>
    </div>
  );
}
