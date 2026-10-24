window.qtable?.stop();
const table = document.querySelector(".quiz-table");
const body = table?.tBodies[0];
if (body) {
    const listen = new AbortController();
    const on = (target, name, handler, options = {}) =>
        target.addEventListener(name, handler, { ...options, signal: listen.signal });
    // How far a pointer may move before it drags (a mouse) or scrolls (a finger); how long a
    // finger holds still before it drags.
    const SLOP = 6;
    const HOLD_MS = 350;
    // How near the top or bottom of the screen the page scrolls along, and how fast at most.
    const EDGE = 60;
    const SPEED = 18;
    const narrow = matchMedia("(max-width: 1300px)");
    let press = null;
    let drag = null;
    let swallow = false;
    let frame = 0;
    let settling = null;
    const cancelPress = () => {
        if (press) clearTimeout(press.timer);
        press = null;
    };
    const begin = () => {
        const all = [...body.rows];
        const { row, id, y } = press;
        const from = all.indexOf(row);
        cancelPress();
        if (from < 0) return;
        settling?.();
        // Where the rows' middles were, on the page rather than the screen, so scrolling keeps
        // them right.
        const mids = all.map((r) => {
            const box = r.getBoundingClientRect();
            return box.top + window.scrollY + box.height / 2;
        });
        drag = { id, all, row, from, to: from, mids, height: row.offsetHeight, start: y + window.scrollY, y };
        try { body.setPointerCapture(id); } catch {}
        table.classList.add("dragging");
        row.classList.add("dragged");
        swallow = true;
        place();
    };
    const place = () => {
        const { all, row, from, mids, height, start } = drag;
        const last = mids.length - 1;
        const shift = Math.max(mids[0], Math.min(mids[last], mids[from] + drag.y + window.scrollY - start)) - mids[from];
        const center = mids[from] + shift;
        let to = from;
        while (to < last && center > mids[to + 1]) to++;
        while (to > 0 && center < mids[to - 1]) to--;
        drag.to = to;
        row.style.transform = `translateY(${shift}px)`;
        all.forEach((other, i) => {
            if (other === row) return;
            const by = i > from && i <= to ? -height : i < from && i >= to ? height : 0;
            other.style.transform = by ? `translateY(${by}px)` : "";
        });
    };
    // Scrolls while the row is held near the top (under the bars held there) or the bottom.
    const scroll = () => {
        frame = 0;
        if (!drag) return;
        const held = narrow.matches ? [".topbar", ".editor-add", ".editor-tools"] : [".topbar"];
        const top = Math.max(0, ...held.map((selector) =>
            document.querySelector(selector)?.getBoundingClientRect().bottom ?? 0)) + EDGE;
        const bottom = window.innerHeight - EDGE;
        const speed = drag.y < top ? -Math.min(1, (top - drag.y) / EDGE)
            : drag.y > bottom ? Math.min(1, (drag.y - bottom) / EDGE) : 0;
        if (!speed) return;
        window.scrollBy({ top: speed * SPEED, behavior: "instant" });
        place();
        frame = requestAnimationFrame(scroll);
    };
    const finish = (drop) => {
        const { all, row, from, to } = drag;
        drag = null;
        cancelAnimationFrame(frame);
        frame = 0;
        table.classList.remove("dragging");
        const reset = () => {
            settling = null;
            for (const r of all) r.style.transform = "";
            row.classList.remove("dragged");
            requestAnimationFrame(() => table.classList.remove("settling"));
        };
        if (!drop || to === from) return reset();
        // The rows stay where they were dropped until the table is redrawn in the new order.
        table.classList.add("settling");
        const redrawn = new MutationObserver(() => settling?.());
        const timer = setTimeout(() => settling?.(), 1000);
        settling = () => {
            redrawn.disconnect();
            clearTimeout(timer);
            reset();
        };
        redrawn.observe(body, { childList: true, subtree: true, characterData: true });
        dioxus.send([from, to]);
    };
    on(body, "pointerdown", (e) => {
        swallow = false;
        if (drag || !e.isPrimary || e.button !== 0) return;
        const row = e.target.closest("tr");
        if (!row) return;
        cancelPress();
        press = { id: e.pointerId, row, x: e.clientX, y: e.clientY, mouse: e.pointerType === "mouse" };
        if (!press.mouse) press.timer = setTimeout(begin, HOLD_MS);
    });
    on(body, "pointermove", (e) => {
        if (drag) {
            if (e.pointerId !== drag.id) return;
            drag.y = e.clientY;
            place();
            if (!frame) scroll();
            return;
        }
        if (!press || e.pointerId !== press.id) return;
        if (Math.hypot(e.clientX - press.x, e.clientY - press.y) <= SLOP) return;
        if (!press.mouse) return cancelPress();
        begin();
        if (!drag) return;
        drag.y = e.clientY;
        place();
    });
    on(window, "pointerup", (e) => {
        if (drag && e.pointerId === drag.id) finish(true);
        else cancelPress();
    });
    on(window, "pointercancel", () => {
        if (drag) finish(false);
        cancelPress();
    });
    on(window, "keydown", (e) => {
        if (drag && e.key === "Escape") finish(false);
    });
    // A held finger would scroll the page, or open the menu for the row's link, instead.
    on(body, "touchmove", (e) => { if (drag) e.preventDefault(); }, { passive: false });
    on(body, "contextmenu", (e) => { if (drag || press) e.preventDefault(); });
    // The click a drop ends with would open the row's card.
    on(table, "click", (e) => {
        if (!swallow) return;
        swallow = false;
        e.preventDefault();
        e.stopPropagation();
    }, { capture: true });
    window.qtable = {
        stop() {
            listen.abort();
            cancelPress();
            cancelAnimationFrame(frame);
            settling?.();
            delete window.qtable;
        },
    };
}
