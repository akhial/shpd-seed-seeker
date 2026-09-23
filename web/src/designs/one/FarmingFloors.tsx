import { useEffect, useId, useState } from "react";
import {
  FARMING_FLOORS,
  isFarmingRequirement,
  toggleFarmingFloor,
} from "../../lib/floor-requirements";
import type { QueryState } from "../../lib/wasm/types";
import { InfoIcon, XIcon } from "../../lib/icons";
import { FeelingSprite } from "./FeelingSprite";

export function FarmingFloors({
  query,
  onChange,
}: {
  query: QueryState;
  onChange: (query: QueryState) => void;
}) {
  const floors = query.floorRequirements ?? [];
  const [helpOpen, setHelpOpen] = useState(false);
  const helpId = useId();

  useEffect(() => {
    if (!helpOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setHelpOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [helpOpen]);

  return (
    <fieldset className="d1-farming-floors">
      <legend>
        <span>Ring of Wealth farming floors</span>
        <span
          className="d1-farming-help"
          onMouseEnter={() => setHelpOpen(true)}
          onMouseLeave={(event) => {
            if (!event.currentTarget.contains(document.activeElement)) setHelpOpen(false);
          }}
          onFocus={() => setHelpOpen(true)}
          onBlur={() => setHelpOpen(false)}
        >
          <button
            type="button"
            className="d1-farming-help-button"
            aria-label="About Ring of Wealth farming floors"
            aria-describedby={helpId}
            onClick={() => setHelpOpen(true)}
          >
            <InfoIcon size={16} />
          </button>
          <span id={helpId} role="tooltip" className="d1-farming-help-tooltip" hidden={!helpOpen}>
            Dark floor with a garden.
          </span>
        </span>
      </legend>
      <div className="d1-farming-options">
        {FARMING_FLOORS.map((depth) => (
          <button
            key={depth}
            type="button"
            className="d1-farming-floor"
            aria-pressed={floors.some(
              (floor) => floor.depth === depth && isFarmingRequirement(floor),
            )}
            onClick={() => onChange(toggleFarmingFloor(query, depth))}
          >
            Floor {depth}
          </button>
        ))}
      </div>
      {floors
        .filter((floor) => !isFarmingRequirement(floor))
        .map((floor) => (
          <div className="d1-floor-filter" key={floor.depth}>
            <span>Floor {floor.depth}</span>
            <FeelingSprite feeling={floor.feeling} />
            {floor.rooms?.map((room) => (
              <span key={room}>{room.replaceAll("_", " ")}</span>
            ))}
            {!!floor.any_rooms?.length && (
              <span>{floor.any_rooms.join(" / ").replaceAll("_", " ")}</span>
            )}
            <button
              type="button"
              className="d1-icon-btn"
              aria-label={`Remove floor ${floor.depth} requirement`}
              onClick={() =>
                onChange({
                  ...query,
                  floorRequirements: floors.filter((entry) => entry.depth !== floor.depth),
                })
              }
            >
              <XIcon size={12} />
            </button>
          </div>
        ))}
    </fieldset>
  );
}
