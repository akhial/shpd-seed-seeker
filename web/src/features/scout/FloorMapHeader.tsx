import { isMapDepthSupported } from "../level-map/client";
import { questLabel, questVariantLabel } from "../../shared/game/quests";
import { regionForDepth } from "../../shared/game/region";
import type { FloorFeeling, ScoutQuest } from "../../engine/types";
import { FeelingSprite } from "../../shared/ui/FeelingSprite";

/** The floor's existing title doubles as its map disclosure. */
export function FloorMapHeader({
  depth,
  feeling,
  quest,
  farming,
  expanded,
  onToggle,
  onPrefetch,
}: {
  depth: number;
  feeling?: FloorFeeling;
  quest?: ScoutQuest;
  farming?: boolean;
  expanded: boolean;
  onToggle: () => void;
  onPrefetch: () => void;
}) {
  const available = isMapDepthSupported(depth);
  const label = (
    <>
      <FloorMapLabel depth={depth} feeling={feeling} quest={quest} idPrefix="scout-floor" />
      {farming && <span className="d1-farm-badge">Garden</span>}
    </>
  );

  if (!available) return <header className="d1-floor-head">{label}</header>;

  return (
    <header className="d1-floor-head d1-floor-head-interactive">
      <button
        type="button"
        id={`scout-floor-map-toggle-${depth}`}
        className="d1-floor-map-toggle"
        aria-expanded={expanded}
        aria-controls={`scout-floor-map-${depth}`}
        aria-label={`${expanded ? "Hide" : "Show"} floor ${depth} map`}
        aria-describedby={`scout-floor-region-${depth}${quest ? ` scout-floor-quest-${depth}` : ""}`}
        onClick={onToggle}
        onKeyDown={(event) => {
          if (expanded && event.key === "Escape") {
            event.stopPropagation();
            onToggle();
          }
        }}
        onPointerEnter={(event) => {
          if (event.pointerType === "mouse") onPrefetch();
        }}
        onFocus={onPrefetch}
      >
        {label}
        <span className="d1-floor-map-action" aria-hidden="true">
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.7"
            strokeLinejoin="round"
          >
            <path d="m3 5 6-2 6 2 6-2v16l-6 2-6-2-6 2V5Z" />
            <path d="M9 3v16M15 5v16" />
          </svg>
          <span>Map</span>
          <svg
            className="d1-floor-map-chevron"
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
      </button>
    </header>
  );
}

/** Shared floor identity for the inline disclosure and expanded map. */
export function FloorMapLabel({
  depth,
  feeling,
  quest,
  idPrefix,
}: {
  depth: number;
  feeling?: FloorFeeling;
  quest?: ScoutQuest;
  idPrefix?: string;
}) {
  const region = regionForDepth(depth);
  return (
    <>
      <span className="d1-floor-bar" aria-hidden="true" />
      <span className="d1-floor-label">Floor {depth}</span>
      <FeelingSprite feeling={feeling} />
      <span className="d1-floor-region" id={idPrefix ? `${idPrefix}-region-${depth}` : undefined}>
        {region.name}
      </span>
      {quest && (
        <span
          className="d1-floor-quest"
          id={idPrefix ? `${idPrefix}-quest-${depth}` : undefined}
          title={`${questLabel(quest.quest)} quest`}
        >
          {questVariantLabel(quest.variant)}
        </span>
      )}
    </>
  );
}
