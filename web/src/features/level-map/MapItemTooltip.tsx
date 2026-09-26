import { useLayoutEffect, useRef, useState } from "react";
import { spriteBoxCss, itemIconCss, spriteGlowCss } from "../../shared/sprites/sprites";
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
      {tip.items.map((item, index) => {
        const sprite = spriteBoxCss(item.image, 32);
        return (
          <article key={index} className="d1-map-item-entry">
            <div className="d1-map-item-heading">
              <span
                className="d1-map-item-icon"
                aria-hidden="true"
                style={{ ...sprite.outer, justifyContent: "flex-start" }}
              >
                <span style={sprite.inner}>
                  {item.glow && (
                    <span
                      className="d1-sprite-glow"
                      style={spriteGlowCss(item.image, 32, [
                        {
                          color: `rgb(${item.glow.color.join(" ")})`,
                          period: item.glow.periodMs / 1000,
                        },
                      ])}
                    />
                  )}
                </span>
                {item.icon && (
                  <span
                    style={{
                      ...itemIconCss(item.icon, 32),
                      right: (32 - Number.parseFloat(String(sprite.inner.width))) / 2,
                    }}
                  />
                )}
              </span>
              <div className="d1-map-item-title">
                <strong>{item.name}</strong>
                {item.upgrade != null && item.upgrade > 0 && (
                  <span
                    className="d1-chip-tag d1-chip-tag-up"
                    aria-label={`Upgrade +${item.upgrade}`}
                  >
                    +{item.upgrade}
                  </span>
                )}
              </div>
              {item.quantity > 1 && <span className="d1-map-item-quantity">×{item.quantity}</span>}
            </div>
            {(item.curse || item.cursed) && (
              <div className="d1-map-item-modifiers">
                <span className="d1-chip-tag d1-badge-curse">
                  {item.cursed ? "Cursed" : "Curse"}
                </span>
              </div>
            )}
            {!item.deterministic && <span className="d1-map-item-context">Varies with play</span>}
            {item.description && <p>{item.description}</p>}
          </article>
        );
      })}
    </div>
  );
}
