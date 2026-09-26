import { useLayoutEffect, useRef, useState } from "react";
import { spriteCss } from "../../shared/sprites/sprites";
import type { MapItemTooltip as ItemTooltip } from "./types";

export function MapItemTooltip({
  tip,
  onPointerEnter,
  id,
  x,
  y,
  width,
  height,
}: {
  tip: ItemTooltip;
  onPointerEnter: () => void;
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ left: 8, top: 8 });
  useLayoutEffect(() => {
    const rect = ref.current?.getBoundingClientRect();
    if (!rect) return;
    const left = Math.max(8, Math.min(x + 18, width - rect.width - 8));
    const top = Math.max(
      8,
      Math.min(
        y + 18 + rect.height <= height - 8 ? y + 18 : y - rect.height - 18,
        height - rect.height - 8,
      ),
    );
    setPosition({ left, top });
  }, [tip, x, y, width, height]);
  return (
    <div
      ref={ref}
      id={id}
      role="tooltip"
      onPointerEnter={onPointerEnter}
      className="d1-map-item-tooltip"
      style={{
        ...position,
        maxWidth: Math.max(0, width - 16),
        maxHeight: Math.max(0, height - 16),
      }}
      onPointerDown={(event) => event.stopPropagation()}
      onPointerMove={(event) => event.stopPropagation()}
    >
      {tip.label && <div className="d1-map-item-context">{tip.label}</div>}
      {tip.items.map((item, index) => (
        <article key={index} className="d1-map-item-entry">
          <div className="d1-map-item-heading">
            <span
              className="d1-map-item-icon"
              aria-hidden="true"
              style={spriteCss(item.image, 32)}
            />
            <strong>{item.name}</strong>
            {item.quantity > 1 && <span className="d1-map-item-quantity">×{item.quantity}</span>}
          </div>
          {!item.deterministic && <span className="d1-map-item-context">Varies with play</span>}
          {item.description && <p>{item.description}</p>}
        </article>
      ))}
    </div>
  );
}
