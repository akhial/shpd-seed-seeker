import { isMapDepthSupported } from "../../lib/level-map/client";
import { questLabel, questVariantLabel } from "../../lib/quests";
import { regionForDepth } from "../../lib/region";
import type { FloorFeeling, ScoutQuest } from "../../lib/wasm/types";
import { FeelingSprite } from "./FeelingSprite";

/** The floor's existing title doubles as its map disclosure. */
export function FloorMapHeader({
  depth,
  feeling,
  quest,
  expanded,
  onToggle,
  onPrefetch,
}: {
  depth: number;
  feeling?: FloorFeeling;
  quest?: ScoutQuest;
  expanded: boolean;
  onToggle: () => void;
  onPrefetch: () => void;
}) {
  const available = isMapDepthSupported(depth);
  const region = regionForDepth(depth);
  const label = (
    <>
      <span className="d1-floor-bar" aria-hidden="true" />
      <span className="d1-floor-label">Floor {depth}</span>
      <FeelingSprite feeling={feeling} />
      <span className="d1-floor-region" id={`scout-floor-region-${depth}`}>
        {region.name}
      </span>
      {quest && (
        <span
          className="d1-floor-quest"
          id={`scout-floor-quest-${depth}`}
          title={`${questLabel(quest.quest)} quest`}
        >
          {questVariantLabel(quest.variant)}
        </span>
      )}
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
