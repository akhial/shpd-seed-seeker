// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { DownloadMenu } from "./DownloadMenu";

let host: HTMLDivElement;
let root: Root;
const fetchRelease = vi.fn();
const releasesURL = "https://github.com/akhial/shpd-seed-seeker/releases/latest";
const instructionsURL =
  "https://github.com/akhial/shpd-seed-seeker/blob/main/ios/README.md#installation";

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("fetch", fetchRelease);
  fetchRelease.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});

async function openIOS() {
  await act(async () => root.render(<DownloadMenu />));
  const platforms = [...host.querySelectorAll<HTMLButtonElement>(".d1-avail-link")];
  const ios = platforms.findIndex((button) => button.textContent === "iOS");
  expect(platforms[ios - 1].textContent).toBe("Android");
  await act(async () => platforms[ios].click());
  expect(host.querySelector('[role="dialog"]')?.getAttribute("aria-label")).toBe(
    "Download Seed Seeker for iOS",
  );
  expect(host.querySelector(`a[href="${instructionsURL}"]`)?.textContent).toContain(
    "Installation instructions",
  );
}

function asset(name: string) {
  return {
    name,
    browser_download_url: `https://github.com/akhial/shpd-seed-seeker/releases/download/v1/${name}`,
  };
}

it("offers the device IPA alongside sideloading instructions instead of downloading immediately", async () => {
  const ipa = asset("seed-seeker-v1-ios-arm64.ipa");
  fetchRelease.mockResolvedValue({
    ok: true,
    json: async () => ({
      tag_name: "v1",
      assets: [
        ipa,
        asset("seed-seeker-cli-v1-ios-arm64.ipa"),
        asset("seed-seeker-v1-ios-arm64.zip"),
        asset("seed-seeker-v1-android-arm64-v8a.apk"),
      ],
    }),
  });
  await openIOS();
  const downloads = host.querySelectorAll<HTMLAnchorElement>("a[download]");
  expect(downloads).toHaveLength(1);
  expect(downloads[0].href).toBe(ipa.browser_download_url);
  expect(host.textContent).toContain("iOS 27 or iPadOS 27");
  expect(host.textContent).toContain("unsigned IPA");
  await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
  expect(host.querySelector('[role="dialog"]')).toBeNull();
  await openIOS();
  expect(fetchRelease).toHaveBeenCalledTimes(1);
});

it.each(["no IPA", "API error", "network error"])(
  "keeps instructions and releases reachable when there is %s",
  async (scenario) => {
    if (scenario === "network error") fetchRelease.mockRejectedValue(new Error("offline"));
    else {
      fetchRelease.mockResolvedValue({
        ok: scenario !== "API error",
        json: async () => ({ tag_name: "v1", assets: [asset("seed-seeker-v1-android.apk")] }),
      });
    }
    await openIOS();
    const fallback = host.querySelector<HTMLAnchorElement>(".d1-dl-list a");
    expect(fallback?.href).toBe(releasesURL);
    expect(fallback?.textContent).toContain("View latest downloads");
    expect(host.querySelector("a[download]")).toBeNull();
  },
);
