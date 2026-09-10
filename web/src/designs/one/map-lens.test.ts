import { describe, expect, it } from "vite-plus/test";
import { lensPosition } from "./map-lens";

describe("scout map inspector placement", () => {
  it("preserves the scout pane even at the narrowest desktop layout", () => {
    const position = lensPosition({
      viewportWidth: 1000,
      viewportHeight: 800,
      paneLeft: 640,
      anchorTop: 220,
      popupHeight: 600,
    });
    expect(position.width).toBe(560);
    expect(position.left + position.width).toBeLessThan(640);
    expect(position.left).toBeGreaterThanOrEqual(12);
    expect(position.top + 600).toBeLessThanOrEqual(788);
  });

  it("keeps a scrolled or low header's inspector inside the viewport", () => {
    for (const anchorTop of [-400, 5, 700, 1400]) {
      const position = lensPosition({
        viewportWidth: 1440,
        viewportHeight: 800,
        paneLeft: 1030,
        anchorTop,
        popupHeight: 600,
      });
      expect(position.top).toBeGreaterThanOrEqual(12);
      expect(position.top + 600).toBeLessThanOrEqual(788);
    }
  });

  it("fits a phone sheet and centers it on a tablet", () => {
    const phone = lensPosition({
      viewportWidth: 375,
      viewportHeight: 667,
      paneLeft: 0,
      anchorTop: 0,
      popupHeight: 550,
    });
    expect(phone).toEqual({ left: 12, top: 105, width: 351 });
    const tablet = lensPosition({
      viewportWidth: 820,
      viewportHeight: 1180,
      paneLeft: 0,
      anchorTop: 0,
      popupHeight: 650,
    });
    expect(tablet).toEqual({ left: 130, top: 518, width: 560 });
  });
});
