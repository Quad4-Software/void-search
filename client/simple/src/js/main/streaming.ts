// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * Progressive results over Server-Sent Events.
 *
 * When the server renders a stream shell, #results carries data-stream-url.
 * "partial" events replace #urls with the ordered results so far; "done"
 * swaps in the full #results block (answers, infoboxes, pagination) and
 * notifies other plugins so they can rebind to the new DOM. Any failure
 * falls back to the classic blocking page (stream=off) exactly once.
 */

const resultsEl = document.querySelector<HTMLElement>("#results[data-stream-url]");
if (resultsEl) {
  const streamUrl = resultsEl.dataset.streamUrl as string;
  const fallbackUrl = resultsEl.dataset.streamOff as string;
  const urlsEl = resultsEl.querySelector<HTMLElement>("#urls");
  const statusEl = resultsEl.querySelector<HTMLElement>(".stream-status");
  const barEl = resultsEl.querySelector<HTMLElement>(".stream-progress-bar");

  let failed = false;
  const source = new EventSource(streamUrl);

  const stop = (): void => {
    source.close();
    resultsEl.removeAttribute("data-stream-url");
    resultsEl.removeAttribute("data-stream-off");
    resultsEl.classList.remove("streaming");
  };

  source.addEventListener("partial", (event: Event) => {
    const data = JSON.parse((event as MessageEvent).data) as { html: string; answered?: number; total?: number };
    if (urlsEl) urlsEl.innerHTML = data.html;
    if (barEl && data.total) barEl.style.width = `${Math.round((100 * (data.answered ?? 0)) / data.total)}%`;
  });

  source.addEventListener("done", (event: Event) => {
    const data = JSON.parse((event as MessageEvent).data) as { html: string; redirect?: string };
    stop();
    if (data.redirect) {
      window.location.href = data.redirect;
      return;
    }
    resultsEl.innerHTML = data.html;
    // let plugins (infinite scroll, calculators, ...) rebind to the new DOM
    document.dispatchEvent(new CustomEvent("searx:results-ready"));
  });

  source.addEventListener("error", () => {
    if (failed) return;
    failed = true;
    stop();
    if (statusEl) statusEl.textContent = "";
    // stream died - reload the blocking page, which never streams twice
    window.location.href = fallbackUrl;
  });

  window.addEventListener("beforeunload", stop);
}
