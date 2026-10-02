// SPDX-License-Identifier: AGPL-3.0-or-later

import { Plugin } from "../Plugin.ts";
import { http, settings } from "../toolkit.ts";


/**
 * Automatically loads the next page when scrolling to bottom of the current page.
 */
export default class InfiniteScroll extends Plugin {
  public constructor() {
    super("infiniteScroll");
  }

  protected async run(): Promise<void> {
    const resultsElement = document.querySelector<HTMLElement>("#results");
    if (!resultsElement) return;

    const onlyImages: boolean = resultsElement.classList.contains("only_template_images");
    const observedSelector = "article.result:last-child";

    const spinnerElement = document.createElement("div");
    spinnerElement.className = "loader";

    const loadNextPage = async (callback: () => void): Promise<void> => {
      const searchForm = document.querySelector<HTMLFormElement>("#search");
      if (!searchForm) return;

      // no next_page form means we are on the last page - nothing to load
      const form = document.querySelector<HTMLFormElement>("#pagination form.next_page");
      if (!form) return;

      const action = searchForm.getAttribute("action");
      if (!action) {
        throw new Error("Form action not defined");
      }

      const paginationElement = document.querySelector<HTMLElement>("#pagination");
      if (!paginationElement) return;

      paginationElement.replaceChildren(spinnerElement);

      try {
        const res = await http("POST", action, { body: new FormData(form) });
        const nextPage = await res.text();
        if (!nextPage) return;

        const nextPageDoc = new DOMParser().parseFromString(nextPage, "text/html");
        const articleList = nextPageDoc.querySelectorAll<HTMLElement>("#urls article");
        const nextPaginationElement = nextPageDoc.querySelector<HTMLElement>("#pagination");

        document.querySelector("#pagination")?.remove();

        const urlsElement = document.querySelector<HTMLElement>("#urls");
        if (!urlsElement) {
          throw new Error("URLs element not found");
        }

        if (articleList.length > 0 && !onlyImages) {
          // do not add <hr> element when there are only images
          urlsElement.appendChild(document.createElement("hr"));
        }

        urlsElement.append(...articleList);

        if (nextPaginationElement) {
          const results = document.querySelector<HTMLElement>("#results");
          results?.appendChild(nextPaginationElement);
          callback();
        }
      } catch (error) {
        console.error("Error loading next page:", error);

        const errorElement = Object.assign(document.createElement("div"), {
          textContent: settings.translations?.error_loading_next_page ?? "Error loading next page",
          className: "dialog-error"
        });
        errorElement.setAttribute("role", "alert");
        document.querySelector("#pagination")?.replaceChildren(errorElement);
      }
    };

    const intersectionObserveOptions: IntersectionObserverInit = {
      rootMargin: "320px"
    };

    const observer: IntersectionObserver = new IntersectionObserver(async (entries: IntersectionObserverEntry[]) => {
      const [paginationEntry] = entries;

      if (paginationEntry?.isIntersecting) {
        observer.unobserve(paginationEntry.target);

        await loadNextPage(() => {
          const nextObservedElement = document.querySelector<HTMLElement>(observedSelector);
          if (nextObservedElement) {
            observer.observe(nextObservedElement);
          }
        });
      }
    }, intersectionObserveOptions);

    const initialObservedElement: HTMLElement | null = document.querySelector<HTMLElement>(observedSelector);
    if (initialObservedElement) {
      observer.observe(initialObservedElement);
    }

    // streamed results swap #results innerHTML once finished - re-observe
    // the (new) last article so the sentinel keeps working
    document.addEventListener("searx:results-ready", () => {
      observer.disconnect();
      const el = document.querySelector<HTMLElement>(observedSelector);
      if (el) observer.observe(el);
    });
  }

  protected async post(): Promise<void> {
    // noop
  }
}
