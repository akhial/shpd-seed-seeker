import { itemArt } from "../../lib/sprites";
import type { TrinketOffer } from "../../lib/wasm/types";
import { Sprite } from "./parts";

export function TrinketShortcuts({
  offers,
  selectedTrinket,
  onSelect,
  disabled = false,
  hidden = false,
  className,
}: {
  offers: readonly TrinketOffer[];
  selectedTrinket?: string | null;
  onSelect: (trinket: string) => void;
  disabled?: boolean;
  hidden?: boolean;
  className: string;
}) {
  return (
    <div
      className={className}
      role="group"
      aria-label="Trinket shortcuts"
      aria-hidden={hidden || undefined}
      inert={hidden || undefined}
    >
      {offers.map((offer) => (
        <button
          key={offer.id}
          type="button"
          className="d1-scout-trinket"
          aria-label={offer.name}
          title={offer.name}
          aria-pressed={selectedTrinket === offer.id}
          disabled={disabled}
          onClick={() => onSelect(selectedTrinket === offer.id ? "none" : offer.id)}
        >
          <Sprite art={itemArt(offer.spriteIndex)} size={18} />
        </button>
      ))}
    </div>
  );
}
