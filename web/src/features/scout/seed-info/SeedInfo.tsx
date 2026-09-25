import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { InfoIcon, XIcon } from "../../../shared/ui/icons";
import type { ItemMappings } from "../../../engine/types";
import artwork from "../../../generated/item-mapping-art.json";
import type { CSSProperties } from "react";
import { ItemMappingTile, mappingLabel } from "./ItemMappingTile";
import "./seed-info.css";

export function SeedInfo({ seed, mappings }: { seed: string; mappings: ItemMappings }) {
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState<string>();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current!;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    dialog.showModal();
    return () => {
      dialog.close();
      document.body.style.overflow = previousOverflow;
      buttonRef.current?.focus({ preventScroll: true });
    };
  }, [open]);

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        className="d1-result-copy d1-seed-info-button"
        aria-label="Seed information"
        aria-haspopup="dialog"
        title="Scroll runes, potion colors, and ring gems"
        onClick={() => {
          setSelected(undefined);
          setOpen(true);
        }}
      >
        <InfoIcon size={18} />
      </button>
      {open &&
        createPortal(
          <dialog
            ref={dialogRef}
            className="d1-seed-info-dialog"
            aria-labelledby="seed-info-title"
            onCancel={() => setOpen(false)}
            onClose={() => setOpen(false)}
            onKeyDown={(event) => event.stopPropagation()}
          >
            <header>
              <div>
                <h2 id="seed-info-title">Seed information</h2>
                <p className="d1-mono">{seed}</p>
              </div>
              <button
                type="button"
                className="d1-result-copy"
                aria-label="Close seed information"
                onClick={() => setOpen(false)}
              >
                <XIcon size={20} />
              </button>
            </header>
            <div
              className="d1-seed-mappings"
              style={
                { "--mapping-units": 6 * artwork.slotSize + 5 * artwork.slotGap } as CSSProperties
              }
            >
              {selected && (
                <p className="d1-mapping-detail" role="status">
                  {selected}
                </p>
              )}
              {(
                [
                  ["potions", "Potions"],
                  ["scrolls", "Scrolls"],
                  ["rings", "Rings"],
                ] as const
              ).map(([category, title]) => (
                <section key={category} aria-label={title}>
                  <h3>{title}</h3>
                  <div className="d1-mapping-grid">
                    {mappings[category].map((entry, classIndex) => (
                      <ItemMappingTile
                        key={entry.name}
                        entry={entry}
                        category={category}
                        classIndex={classIndex}
                        selected={selected === mappingLabel(entry)}
                        onSelect={() =>
                          setSelected(
                            selected === mappingLabel(entry) ? undefined : mappingLabel(entry),
                          )
                        }
                      />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </dialog>,
          document.body,
        )}
    </>
  );
}
