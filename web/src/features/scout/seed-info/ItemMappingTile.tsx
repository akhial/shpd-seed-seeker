import type { CSSProperties } from "react";
import artwork from "../../../generated/item-mapping-art.json";
import type { ItemMapping, ItemMappings } from "../../../engine/types";

export const mappingLabel = (entry: ItemMapping) => `${entry.appearance} — ${entry.name}`;

/** Journal frames retain their transparent padding and share one pixel scale. */
function frameStyle(
  sheet: string,
  index: number,
  cellSize: number,
  width: number,
  height: number,
  left: number,
  top: number,
): CSSProperties {
  const pixels = (value: number) => `calc(${value} * var(--mapping-pixel))`;
  return {
    left: pixels(left),
    top: pixels(top),
    width: pixels(width),
    height: pixels(height),
    backgroundImage: `url(/third_party/shattered-pixel-dungeon/${sheet})`,
    backgroundSize: `${pixels(16 * cellSize)} auto`,
    backgroundPosition: `${pixels(-(index % 16) * cellSize)} ${pixels(-Math.floor(index / 16) * cellSize)}`,
  };
}

export function ItemMappingTile({
  entry,
  category,
  classIndex,
  selected,
  onSelect,
}: {
  entry: ItemMapping;
  category: keyof ItemMappings;
  classIndex: number;
  selected: boolean;
  onSelect: () => void;
}) {
  const art = artwork.categories[category];
  const [width, height] = art.spriteSize;
  const [iconWidth, iconHeight] = art.iconSizes[classIndex];
  const label = mappingLabel(entry);
  return (
    <button
      type="button"
      className="d1-mapping-tile"
      aria-label={label}
      aria-pressed={selected}
      title={label}
      onClick={onSelect}
    >
      <span
        aria-hidden="true"
        className="d1-mapping-item"
        style={frameStyle(
          "items.png",
          entry.spriteIndex,
          16,
          width,
          height,
          (artwork.slotSize - width) / 2,
          (artwork.slotSize - height) / 2,
        )}
      />
      <span
        aria-hidden="true"
        className="d1-mapping-identity"
        style={frameStyle(
          "item_icons.png",
          art.iconBase + classIndex,
          8,
          iconWidth,
          iconHeight,
          artwork.slotSize - iconWidth,
          0,
        )}
      />
    </button>
  );
}
