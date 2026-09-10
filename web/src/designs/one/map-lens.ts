/** Keep the scout visible on desktop; fit the same inspector above mobile chrome. */
export function lensPosition({
  viewportWidth,
  viewportHeight,
  paneLeft,
  anchorTop,
  popupHeight,
}: {
  viewportWidth: number;
  viewportHeight: number;
  paneLeft: number;
  anchorTop: number;
  popupHeight: number;
}): { left: number; top: number; width: number } {
  const gap = 12;
  const mobile = viewportWidth < 1_000;
  const width = mobile ? Math.min(560, viewportWidth - gap * 2) : Math.min(560, paneLeft - gap * 2);
  return {
    width: Math.max(0, width),
    left: mobile ? (viewportWidth - width) / 2 : Math.max(gap, paneLeft - width - gap),
    top: mobile
      ? Math.max(gap, viewportHeight - popupHeight - gap)
      : Math.max(gap, Math.min(anchorTop - gap, viewportHeight - popupHeight - gap)),
  };
}
