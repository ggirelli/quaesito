window.qnav?.stop();
const list = document.querySelector(".editor-list");
const nav = document.querySelector(".editor-nav");
if (list && nav) {
    const root = document.documentElement;
    const topbar = document.querySelector(".topbar");
    // Below this width the navigator lies across the top of the list instead of down the side.
    const narrow = matchMedia("(max-width: 1300px)");
    const listen = new AbortController();
    const on = (target, name, handler, options = {}) =>
        target.addEventListener(name, handler, { ...options, signal: listen.signal });
    // Where cards come into view: under the header, and on narrow screens under the navigator
    // and the buttons held below it.
    const held = [".topbar", ".editor-nav", ".editor-add", ".editor-tools"];
    // Measured where they stick, not where they are: near the top of the page they are lower.
    const offset = () => Math.max(0, ...(narrow.matches ? held : held.slice(0, 1)).map((selector) => {
        const element = document.querySelector(selector);
        return element ? (parseFloat(getComputedStyle(element).top) || 0) + element.offsetHeight : 0;
    })) + 8;
    // The cards, or the table's rows in their place.
    const cards = () => [...list.querySelectorAll(":scope > [id], tbody > tr[id]")];
    const chips = () => [...nav.children];
    let following = null;
    let shown = null;
    // The cards on screen, as the first and last index; and where the pointer points, if anywhere.
    let onScreen = [0, -1];
    let pointer = null;
    let scrubbing = false;
    let scrubbedAt = 0;
    const align = (card, behavior) => window.scrollTo({
        top: card.getBoundingClientRect().top + window.scrollY - offset(),
        behavior,
    });
    const layout = () => {
        const all = chips();
        const count = all.length;
        if (!count) return;
        const across = narrow.matches;
        const length = across ? nav.clientWidth : nav.clientHeight;
        // Tighter where many links share little room, as on a phone.
        const gap = count * 4 > length ? 1 : 2;
        nav.style.gap = `${gap}px`;
        const room = length - gap * (count - 1);
        const thinnest = 2;
        let open = across ? 40 : 30;
        // Fully open within `reach` of the focus, then narrowing over three more.
        const [first, last, reach] = pointer === null
            ? [...onScreen, 2]
            : [Math.floor(pointer), Math.floor(pointer), 3];
        const sizes = all.map((_, i) => {
            if (count * open <= room) return 1;
            const away = i < first ? first - i : i > last ? i - last : 0;
            return Math.max(0, Math.min(1, 1 - (away - reach) / 3));
        });
        const opened = sizes.reduce((sum, size) => sum + size, 0);
        open = Math.min(open, (room - thinnest * (count - opened)) / opened);
        const thin = opened >= count ? 0 : Math.max(thinnest, (room - open * opened) / (count - opened));
        all.forEach((chip, i) => {
            chip.style.flexBasis = `${thin + (open - thin) * sizes[i]}px`;
            chip.toggleAttribute("data-open", sizes[i] === 1);
        });
    };
    const update = () => {
        const top = offset();
        const all = chips();
        let current = -1;
        onScreen = [0, -1];
        cards().forEach((card, i) => {
            const at = card.getBoundingClientRect();
            const visible = at.bottom > top && at.top < window.innerHeight;
            all[i]?.toggleAttribute("data-visible", visible);
            all[i]?.removeAttribute("aria-current");
            if (visible) onScreen = [onScreen[1] < 0 ? i : onScreen[0], i];
            if (at.top <= top + 1) current = i;
        });
        all[current]?.setAttribute("aria-current", "location");
        layout();
        const id = cards()[current]?.id ?? null;
        if (id === shown) return;
        shown = id;
        const url = shown ? `#${shown}` : location.pathname + location.search;
        history.replaceState(history.state, "", url);
    };
    let frame = 0;
    const schedule = () => {
        if (!frame) frame = requestAnimationFrame(() => {
            frame = 0;
            if (following) align(following, "instant");
            update();
        });
    };
    const letGo = () => { following = null; };
    // The pointer's place along the strip, in links, as if they were all the same size: the open
    // part then stays put under it instead of sliding away as the links resize.
    const along = (e) => {
        const box = nav.getBoundingClientRect();
        const share = narrow.matches
            ? (e.clientX - box.left) / box.width
            : (e.clientY - box.top) / box.height;
        return Math.max(0, Math.min(0.999, share)) * nav.children.length;
    };
    on(nav, "pointermove", (e) => { pointer = along(e); schedule(); });
    on(nav, "pointerleave", () => { if (!scrubbing) { pointer = null; schedule(); } });
    // A finger slides along the strip, opening it where it goes, and jumps where it lifts.
    on(nav, "pointerdown", (e) => {
        if (e.pointerType === "mouse") return;
        scrubbing = true;
        nav.setPointerCapture(e.pointerId);
        pointer = along(e);
        schedule();
    });
    on(nav, "pointerup", (e) => {
        if (!scrubbing) return;
        scrubbing = false;
        scrubbedAt = Date.now();
        const chip = document.elementFromPoint(e.clientX, e.clientY)?.closest(".editor-nav a");
        const card = cards()[chip ? chips().indexOf(chip) : Math.floor(along(e))];
        pointer = null;
        letGo();
        if (card) align(card, "smooth");
        schedule();
    });
    on(nav, "pointercancel", () => { scrubbing = false; pointer = null; schedule(); });
    // Tabbing through the links opens the strip around the focused one. A click focuses a link
    // too, but there the pointer already steers, and moving the link would lose the click.
    on(nav, "focusin", (e) => {
        if (!e.target.matches(":focus-visible")) return;
        pointer = chips().indexOf(e.target.closest("a"));
        schedule();
    });
    on(nav, "focusout", () => { if (!nav.matches(":hover")) { pointer = null; schedule(); } });
    // The heights the held bars stack on, kept for the CSS.
    const add = document.querySelector(".editor-add");
    const sizes = new ResizeObserver(() => {
        root.style.setProperty("--topbar-height", `${topbar?.offsetHeight ?? 0}px`);
        root.style.setProperty("--editor-add-height", `${add?.offsetHeight ?? 0}px`);
        schedule();
    });
    if (topbar) sizes.observe(topbar);
    if (add) sizes.observe(add);
    sizes.observe(list);
    sizes.observe(nav);
    const changes = new MutationObserver(schedule);
    changes.observe(list, { childList: true, subtree: true, attributeFilter: ["id"] });
    changes.observe(nav, { childList: true });
    for (const name of ["wheel", "touchstart", "keydown", "pointerdown"]) {
        on(window, name, letGo, { passive: true });
    }
    on(window, "scroll", schedule, { passive: true });
    on(window, "resize", schedule);
    const timer = setTimeout(letGo, 3000);
    const id = decodeURIComponent(location.hash.slice(1));
    const target = id && document.getElementById(id);
    if (target && list.contains(target)) following = target;
    requestAnimationFrame(() => requestAnimationFrame(schedule));
    window.qnav = {
        jump(id, behavior = "smooth") {
            // The click that follows a finger lifted from the strip.
            if (Date.now() - scrubbedAt < 500) return;
            const card = document.getElementById(id);
            following = null;
            if (card) align(card, behavior);
        },
        stop() {
            listen.abort();
            sizes.disconnect();
            changes.disconnect();
            clearTimeout(timer);
            cancelAnimationFrame(frame);
            delete window.qnav;
        },
    };
}
