import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { InfoIcon, XIcon } from "../../lib/icons";
import type { ItemMappings } from "../../lib/wasm/types";
import { Sprite } from "./parts";
import "./seed-info.css";

export function SeedInfo({ seed, mappings }: { seed: string; mappings: ItemMappings }) {
  const [open, setOpen] = useState(false);
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
        onClick={() => setOpen(true)}
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
            <div className="d1-seed-mappings">
              {(
                [
                  ["scrolls", "Scroll runes"],
                  ["potions", "Potion colors"],
                  ["rings", "Ring gems"],
                ] as const
              ).map(([category, title]) => (
                <section key={category} aria-label={title}>
                  <h3>{title}</h3>
                  <dl>
                    {[...mappings[category]]
                      .sort((a, b) => a.appearance.localeCompare(b.appearance))
                      .map((entry) => (
                        <div key={entry.appearance} className="d1-seed-mapping">
                          <dt>
                            <Sprite art={{ cell: entry.spriteIndex }} size={28} />
                            <span>{entry.appearance}</span>
                          </dt>
                          <dd>{entry.name}</dd>
                        </div>
                      ))}
                  </dl>
                </section>
              ))}
            </div>
          </dialog>,
          document.body,
        )}
    </>
  );
}
