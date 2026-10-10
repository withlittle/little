// The built-in theme's script. Every page works without it: it adds a toggle
// between light and dark, a button to copy each code block, and marks where
// the reader is in the table of contents.
(() => {
  const root = document.documentElement;

  // Light or dark: the system's, unless the reader chooses otherwise.
  // Choosing what the system shows forgets the choice, so the page follows
  // the system again.
  const toggle = document.querySelector(".theme-toggle");
  const systemDark = matchMedia("(prefers-color-scheme: dark)");
  const system = () => (systemDark.matches ? "dark" : "light");
  const current = () => root.dataset.theme ?? system();
  const sync = () => toggle?.setAttribute("aria-pressed", String(current() === "dark"));
  toggle?.addEventListener("click", () => {
    const theme = current() === "dark" ? "light" : "dark";
    if (theme === system()) delete root.dataset.theme;
    else root.dataset.theme = theme;
    try {
      if (theme === system()) localStorage.removeItem("little-theme");
      else localStorage.setItem("little-theme", theme);
    } catch {}
    sync();
  });
  systemDark.addEventListener("change", sync);
  sync();

  // The nav is a drawer only where there's no room for it beside the page.
  const nav = document.getElementById("_nav");
  const wide = matchMedia("(width >= 60rem)");
  wide.addEventListener("change", () => {
    if (wide.matches && nav?.matches(":popover-open")) nav.hidePopover();
  });

  // The table of contents folded above the page folds again once the reader
  // picks a heading.
  const tocInline = document.querySelector(".toc-inline");
  tocInline?.addEventListener("click", (event) => {
    if (event.target.closest("a")) tocInline.open = false;
  });

  // A button to copy each code block, but not a notebook's output. The
  // clipboard is there only on HTTPS, and localhost.
  const icon = (paths) =>
    `<svg class="icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${paths}</svg>`;
  const copyIcon = icon('<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"/>');
  const copiedIcon = icon('<path d="M5 12.5l4.5 4.5L19 7.5"/>');
  if (navigator.clipboard) {
    const status = document.createElement("span");
    status.className = "visually-hidden";
    status.setAttribute("role", "status");
    document.body.append(status);
    for (const pre of document.querySelectorAll(".content pre")) {
      if (pre.querySelector(":scope > code.language-output")) continue;
      const block = document.createElement("div");
      block.className = "code-block";
      pre.before(block);
      block.append(pre);
      const button = document.createElement("button");
      button.type = "button";
      button.className = "icon-button copy-button";
      button.setAttribute("aria-label", "Copy code");
      button.innerHTML = copyIcon;
      block.append(button);
      let timer;
      button.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(pre.textContent.replace(/\n$/, ""));
        } catch {
          return;
        }
        button.innerHTML = copiedIcon;
        button.dataset.copied = "";
        status.textContent = "Copied";
        clearTimeout(timer);
        timer = setTimeout(() => {
          button.innerHTML = copyIcon;
          delete button.dataset.copied;
          status.textContent = "";
        }, 2000);
      });
    }
  }

  // Mark the heading the reader is at in the table of contents beside the
  // page: the last one above the top of the window, or the last of all at
  // the bottom of the page.
  const fragment = (link) => {
    try {
      return decodeURIComponent(link.hash.slice(1));
    } catch {
      return link.hash.slice(1);
    }
  };
  const items = [...document.querySelectorAll(".toc a")]
    .map((link) => [link, document.getElementById(fragment(link))])
    .filter(([, heading]) => heading);
  if (items.length) {
    let marked;
    const update = () => {
      const top = (parseFloat(getComputedStyle(root).scrollPaddingTop) || 0) + 1;
      const bottom = innerHeight + scrollY >= root.scrollHeight - 2;
      let item = bottom ? items.at(-1) : undefined;
      if (!item) {
        for (const candidate of items) {
          if (candidate[1].getBoundingClientRect().top > top) break;
          item = candidate;
        }
      }
      const link = item?.[0];
      if (link === marked) return;
      marked?.removeAttribute("data-current");
      link?.setAttribute("data-current", "");
      marked = link;
    };
    let queued = false;
    const queue = () => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => {
        queued = false;
        update();
      });
    };
    addEventListener("scroll", queue, { passive: true });
    addEventListener("resize", queue);
    update();
  }
})();
