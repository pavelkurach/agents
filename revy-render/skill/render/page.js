// The page's one script, inlined in <head>. Progressive enhancement only: without it every link still navigates,
// every tip is a native title, and every section and diagram is complete.
(() => {
  const root = document.documentElement;
  const KEY = 'revy-theme';
  const stored = () => {
    try {
      const t = localStorage.getItem(KEY);
      return t === 'light' || t === 'dark' ? t : null;
    } catch {
      return null;
    }
  };
  const initial = stored();
  if (initial) root.dataset.theme = initial;
  addEventListener('beforeprint', () => document.querySelectorAll('details:not([open])').forEach((d) => { d.open = true; }));
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');

  /** Theme: the system's until the reader picks one, which is remembered. */
  function themeToggles() {
    const system = matchMedia('(prefers-color-scheme: dark)');
    const current = () => root.dataset.theme || (system.matches ? 'dark' : 'light');
    const SUN = '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="3" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M8 1v2M8 13v2M1 8h2M13 8h2M3 3l1.4 1.4M11.6 11.6L13 13M3 13l1.4-1.4M11.6 4.4L13 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>';
    const MOON = '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13.5 9.5A5.5 5.5 0 0 1 6.5 2.5a5.5 5.5 0 1 0 7 7z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/></svg>';
    const buttons = [];
    const paint = () => buttons.forEach((b) => {
      const dark = current() === 'dark';
      b.innerHTML = dark ? SUN : MOON;
      b.setAttribute('aria-label', dark ? 'Switch to light theme' : 'Switch to dark theme');
      b.title = b.getAttribute('aria-label');
    });
    const make = () => {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'theme-toggle';
      b.addEventListener('click', () => {
        const next = current() === 'dark' ? 'light' : 'dark';
        root.dataset.theme = next;
        try { localStorage.setItem(KEY, next); } catch { /* storage blocked: the choice lasts this visit */ }
        paint();
      });
      buttons.push(b);
      return b;
    };
    system.addEventListener('change', paint);
    return { make, paint };
  }

  const HEADING = 'h1, h2, h3, h4, h5, h6';
  const words = (el) => el.textContent.replace(/\s+/g, ' ').trim();
  const para = (cls, text) => {
    const p = document.createElement('p');
    p.className = cls;
    p.textContent = text.length > 180 ? `${text.slice(0, 179)}…` : text;
    return p;
  };

  /**
   * What a link inside the page points at, in brief: a heading or section by its title and first line, a figure by
   * its caption and a small copy of its drawing, anything else (a definition, a footnote) by its own first line.
   */
  function previewOf(target) {
    if (target.matches(HEADING)) {
      const next = target.nextElementSibling;
      return [para('tip-title', words(target)), ...(next ? [para('tip-line', words(next))] : [])];
    }
    const head = target.querySelector(`${HEADING}, figcaption`) ?? target.querySelector('summary');
    const svg = target.matches('figure') && target.querySelector('svg');
    if (svg) {
      const thumb = document.createElement('div');
      thumb.className = 'tip-thumb';
      const copy = svg.cloneNode(true);
      copy.querySelectorAll('[id]').forEach((n) => n.removeAttribute('id'));
      copy.removeAttribute('id');
      thumb.append(copy);
      return [...(head ? [para('tip-title', words(head))] : []), thumb];
    }
    if (!head) {
      const copy = target.cloneNode(true);
      copy.querySelectorAll('.footnote-backref').forEach((n) => n.remove());
      return [para('tip-line', words(copy))];
    }
    const line = [...target.querySelectorAll('p, li, dd, td')].find((el) => !head.contains(el) && !el.closest('figure, pre, .tip'));
    return [para('tip-title', words(head)), ...(line ? [para('tip-line', words(line))] : [])];
  }

  /**
   * One popup system. A term, evidence chip, history ref, outside link or diagram node shows its text; a link inside the
   * page previews its target; a code chunk shows a copy of its note, whose links stay live, and stays open while the
   * pointer moves into it. A tip opened from inside a tip stacks above it, so a term in a chunk's note still explains
   * itself.
   */
  function tips() {
    const SOURCES = 'a[data-tip], a[data-preview], .chunk[data-note]';
    const layers = [0, 1].map((level) => {
      const tip = document.createElement('div');
      tip.className = 'tip';
      tip.id = level ? 'revy-tip-2' : 'revy-tip';
      tip.setAttribute('role', 'tooltip');
      tip.hidden = true;
      document.body.append(tip);
      return { tip, owner: null, timer: 0 };
    });
    const levelOf = (el) => (el.closest('.tip') ? 1 : 0);
    const hideFrom = (level) => layers.slice(level).forEach((l) => {
      clearTimeout(l.timer);
      if (!l.owner) return;
      if (l.owner.dataset.note) [l.owner, document.getElementById(l.owner.dataset.note)].forEach((n) => n?.classList.remove('is-lit'));
      else l.owner.removeAttribute('aria-describedby');
      l.owner = null;
      l.tip.hidden = true;
    });
    const fill = (tip, el) => {
      const note = el.dataset.note && document.getElementById(el.dataset.note);
      tip.classList.toggle('tip-note', Boolean(note));
      if (el.dataset.preview !== undefined) {
        tip.replaceChildren(...previewOf(document.getElementById(decodeURIComponent(el.getAttribute('href').slice(1)))));
        el.setAttribute('aria-describedby', tip.id);
        return;
      }
      // A term or evidence chip shows its definition or ledger entry as written, its code still code; the text tip
      // stands in when the page has no such entry.
      const entry = el.matches('a.term, a.ev, a.node-term') && document.getElementById(decodeURIComponent(el.getAttribute('href').slice(1)));
      const body = entry && (entry.querySelector(':scope > dd') ?? (entry.matches('li') ? entry : null));
      if (!note && body) {
        const copy = body.cloneNode(true);
        copy.querySelectorAll('[id]').forEach((n) => n.removeAttribute('id'));
        tip.replaceChildren(...copy.childNodes);
        el.setAttribute('aria-describedby', tip.id);
        return;
      }
      if (!note) {
        tip.textContent = el.dataset.tip;
        el.setAttribute('aria-describedby', tip.id);
        return;
      }
      const copy = note.cloneNode(true);
      copy.querySelectorAll('[id]').forEach((n) => n.removeAttribute('id'));
      tip.replaceChildren(...copy.childNodes);
      [el, note].forEach((n) => n.classList.add('is-lit'));
    };
    const show = (el) => {
      const level = levelOf(el);
      const l = layers[level];
      clearTimeout(l.timer);
      if (l.owner === el) return;
      hideFrom(level);
      l.owner = el;
      fill(l.tip, el);
      l.tip.hidden = false;
      const r = el.getBoundingClientRect();
      const w = l.tip.offsetWidth;
      const h = l.tip.offsetHeight;
      const left = Math.max(8, Math.min(r.left + r.width / 2 - w / 2, innerWidth - w - 8));
      const below = r.bottom + 8 + h <= innerHeight;
      l.tip.style.left = `${left}px`;
      l.tip.style.top = `${below ? r.bottom + 8 : Math.max(8, r.top - h - 8)}px`;
    };
    // Leaving a chunk for its note's tip must not close it, so closing waits a moment for the pointer to arrive.
    const later = (level) => {
      clearTimeout(layers[level].timer);
      layers[level].timer = setTimeout(() => hideFrom(level), 160);
    };
    const keep = (el) => layers.forEach((l) => { if (l.tip.contains(el) || l.owner?.contains(el)) clearTimeout(l.timer); });
    // Outside links open in a new tab that gets no handle on this page.
    document.querySelectorAll('a[href^="http://"]:not([class]), a[href^="https://"]:not([class])').forEach((a) => {
      a.target = '_blank';
      a.rel = 'noopener';
      a.dataset.tip = `${new URL(a.href).host}${words(a) ? ` — ${words(a)}` : ''}`;
    });
    // A diagram node that names a term or code explains itself like the same words in prose; only an explicit link
    // (a Mermaid `click` or anchor) previews where it goes.
    document.querySelectorAll(':is(main, .masthead) a[href^="#"]:not(.ln, .ev, .term, .node-link, .footnote-backref)').forEach((a) => {
      const id = decodeURIComponent(a.getAttribute('href').slice(1));
      if (id && document.getElementById(id)) a.dataset.preview = '';
    });
    document.querySelectorAll('a.term[title], a.ev[title], a.ref[title], a.node-link').forEach((el) => {
      const title = el.querySelector(':scope > title');
      el.dataset.tip = title ? title.textContent : el.getAttribute('title');
      if (title) {
        title.remove();
        el.setAttribute('aria-label', `${textOf(el)}: ${el.dataset.tip}`);
      } else el.removeAttribute('title');
    });
    document.addEventListener('mouseover', (e) => {
      keep(e.target);
      const el = e.target.closest(SOURCES);
      if (el) show(el);
    });
    document.addEventListener('mouseout', (e) => {
      [e.target.closest(SOURCES), e.target.closest('.tip')]
        .filter((from) => from && !from.contains(e.relatedTarget))
        .forEach((from) => layers.forEach((l, level) => { if (l.owner === from || l.tip === from) later(level); }));
    });
    document.addEventListener('focusin', (e) => {
      const el = e.target.closest(SOURCES);
      if (el) show(el);
    });
    document.addEventListener('focusout', (e) => {
      layers.forEach((l, level) => { if (l.owner === e.target && !l.tip.contains(e.relatedTarget)) hideFrom(level); });
    });
    addEventListener('keydown', (e) => { if (e.key === 'Escape') hideFrom(0); });
    addEventListener('scroll', () => hideFrom(0), { passive: true });
  }

  /** A link into a closed disclosure (evidence entry, FAQ entry, folded section) opens it first. */
  function openTargets() {
    const reveal = () => {
      const id = decodeURIComponent(location.hash.slice(1));
      const el = id && document.getElementById(id);
      if (!el) return;
      for (let d = el.closest('details'); d; d = d.parentElement.closest('details')) d.open = true;
      el.scrollIntoView({ block: 'start', behavior: reduced.matches ? 'auto' : 'smooth' });
    };
    addEventListener('hashchange', reveal);
    document.addEventListener('click', (e) => {
      const a = e.target.closest('a[href^="#"]');
      const el = a && document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1)));
      if (el) for (let d = el.closest('details'); d; d = d.parentElement.closest('details')) d.open = true;
    });
    if (location.hash) reveal();
  }

  /** The navigation marks the section being read; below the wide layout a compact bar takes over once it scrolls away. */
  function navigation(theme) {
    const toc = document.querySelector('.toc');
    if (!toc) {
      document.querySelector('.eyebrow')?.append(theme.make());
      return;
    }
    toc.querySelector('.toc-title').append(theme.make());
    const links = [...toc.querySelectorAll('a[href^="#"]')];
    const targets = links.map((a) => document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1))));
    const bar = document.createElement('div');
    bar.className = 'compact-bar';
    bar.innerHTML = '<span class="cb-current"></span><button type="button" class="cb-menu" aria-expanded="false">Sections</button>';
    const menu = toc.querySelector('ol').cloneNode(true);
    menu.querySelectorAll('.toc-sub').forEach((s) => s.remove());
    menu.hidden = true;
    menu.id = 'revy-sections';
    bar.querySelector('.cb-menu').setAttribute('aria-controls', menu.id);
    bar.append(menu, theme.make());
    document.body.append(bar);
    const button = bar.querySelector('.cb-menu');
    const close = () => { menu.hidden = true; button.setAttribute('aria-expanded', 'false'); };
    button.addEventListener('click', () => { menu.hidden = !menu.hidden; button.setAttribute('aria-expanded', String(!menu.hidden)); });
    menu.addEventListener('click', close);
    addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
    const barLinks = [...menu.querySelectorAll('a')];
    new IntersectionObserver(([e]) => {
      bar.classList.toggle('is-shown', !e.isIntersecting && e.boundingClientRect.top < 0);
      if (e.isIntersecting) close();
    }).observe(toc);
    let pending = false;
    const spy = () => {
      pending = false;
      const line = innerHeight * 0.3;
      let current = -1;
      targets.forEach((t, i) => {
        if (t && t.offsetParent !== null && t.getBoundingClientRect().top <= line) current = i;
      });
      if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) current = targets.findLastIndex((t) => t && t.offsetParent !== null);
      links.forEach((a, i) => {
        a.classList.toggle('is-current', i === current);
        if (i === current) a.setAttribute('aria-current', 'location'); else a.removeAttribute('aria-current');
      });
      const href = current >= 0 ? links[current].getAttribute('href') : null;
      barLinks.forEach((a) => a.classList.toggle('is-current', a.getAttribute('href') === href));
      bar.querySelector('.cb-current').textContent = current >= 0 ? links[current].textContent : document.title;
    };
    addEventListener('scroll', () => { if (!pending) { pending = true; requestAnimationFrame(spy); } }, { passive: true });
    document.addEventListener('toggle', spy, true);
    spy();
  }

  /** Hovering or focusing a snippet note lights the lines it names. */
  function snippetNotes() {
    document.querySelectorAll('.snippet-notes').forEach((list) => {
      const fig = document.getElementById(list.dataset.snippet);
      if (!fig) return;
      list.querySelectorAll('li[data-lines]').forEach((li) => {
        const lines = li.dataset.lines.split(',').flatMap((r) => {
          const [a, b = a] = r.split('-').map(Number);
          return Array.from({ length: b - a + 1 }, (_, k) => a + k);
        });
        const set = (on) => {
          li.classList.toggle('is-lit', on);
          const side = li.dataset.side === 'base' ? 'data-base' : 'data-line';
          lines.forEach((n) => fig.querySelector(`.line[${side}="${n}"]`)?.classList.toggle('is-lit', on));
        };
        li.addEventListener('mouseenter', () => set(true));
        li.addEventListener('mouseleave', () => set(false));
        li.addEventListener('focusin', () => set(true));
        li.addEventListener('focusout', () => set(false));
      });
    });
  }

  /**
   * Table columns resize by a handle on each header's right edge (drag, arrow keys, double-click to fit the content on
   * one line). A resized table remembers its widths in this browser and offers to reset them.
   */
  function tableColumns() {
    const STEP = 16;
    const MIN = 48;
    document.querySelectorAll('.table-wrap > table').forEach((table, index) => {
      const heads = [...table.querySelectorAll(':scope > thead > tr:last-child > th')];
      if (!heads.length) return;
      const key = `revy-cols:${location.pathname}:${index}`;
      const reset = document.createElement('button');
      reset.type = 'button';
      reset.className = 'table-reset';
      reset.textContent = 'Reset widths';
      reset.hidden = true;
      table.parentElement.after(reset);
      const widths = () => heads.map((th) => parseFloat(th.style.width) || th.getBoundingClientRect().width);
      // A fixed layout from the current widths lets one column change without the others reflowing.
      const apply = (ws) => {
        heads.forEach((th, k) => {
          th.style.width = `${ws[k]}px`;
          th.querySelector('.col-handle')?.setAttribute('aria-valuenow', String(Math.round(ws[k])));
        });
        table.style.width = `${ws.reduce((a, b) => a + b, 0)}px`;
        table.style.tableLayout = 'fixed';
        reset.hidden = false;
      };
      const save = () => {
        try { localStorage.setItem(key, JSON.stringify(widths())); } catch { /* storage blocked: widths last this visit */ }
      };
      const set = (k, w) => {
        const ws = widths();
        ws[k] = Math.max(MIN, Math.round(w));
        apply(ws);
      };
      const fit = (k) => {
        const cells = [...table.rows].map((r) => r.cells[k]).filter(Boolean);
        set(k, MIN);
        cells.forEach((c) => { c.style.whiteSpace = 'nowrap'; });
        const w = Math.max(...cells.map((c) => c.scrollWidth)) + 2;
        cells.forEach((c) => { c.style.whiteSpace = ''; });
        set(k, w);
        save();
      };
      heads.forEach((th, k) => {
        const handle = document.createElement('span');
        handle.className = 'col-handle';
        handle.tabIndex = 0;
        handle.setAttribute('role', 'separator');
        handle.setAttribute('aria-orientation', 'vertical');
        handle.setAttribute('aria-valuemin', String(MIN));
        handle.setAttribute('aria-valuenow', String(Math.round(th.getBoundingClientRect().width)));
        handle.setAttribute('aria-label', `Resize column ${th.textContent.replace(/\s+/g, ' ').trim()}`);
        handle.addEventListener('keydown', (e) => {
          if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
          e.preventDefault();
          set(k, widths()[k] + (e.key === 'ArrowRight' ? STEP : -STEP));
          save();
        });
        handle.addEventListener('pointerdown', (e) => {
          e.preventDefault();
          const startX = e.clientX;
          const start = widths()[k];
          handle.setPointerCapture(e.pointerId);
          handle.classList.add('is-dragging');
          const move = (ev) => set(k, start + ev.clientX - startX);
          const up = () => {
            handle.classList.remove('is-dragging');
            handle.removeEventListener('pointermove', move);
            save();
          };
          handle.addEventListener('pointermove', move);
          handle.addEventListener('pointerup', up, { once: true });
        });
        handle.addEventListener('dblclick', () => fit(k));
        th.append(handle);
      });
      reset.addEventListener('click', () => {
        heads.forEach((th) => { th.style.width = ''; });
        table.style.width = '';
        table.style.tableLayout = '';
        reset.hidden = true;
        try { localStorage.removeItem(key); } catch { /* storage blocked: nothing was kept */ }
      });
      try {
        const saved = JSON.parse(localStorage.getItem(key) ?? 'null');
        if (Array.isArray(saved) && saved.length === heads.length && saved.every((n) => typeof n === 'number' && n > 0)) apply(saved);
      } catch { /* unreadable or blocked storage: the default widths stand */ }
    });
  }

  /**
   * Hovering or focusing a flowchart node or state lights its edges and neighbours and dims the rest; hovering an edge
   * or its label lights it and the two nodes it joins.
   */
  function tracing() {
    document.querySelectorAll('svg.pele').forEach((svg) => {
      const edges = [...svg.querySelectorAll('.pele-edge[data-from]')];
      if (!edges.length) return;
      const nodes = [...svg.querySelectorAll('.pele-node')];
      const labels = (e) => labelsOf(svg, e.dataset.id);
      const light = (hosts, lit) => {
        const set = (on) => {
          svg.classList.toggle('is-tracing', on);
          lit.forEach((el) => el.classList.toggle('is-lit', on));
        };
        hosts.forEach((host) => {
          host.addEventListener('mouseenter', () => set(true));
          host.addEventListener('mouseleave', () => set(false));
          host.addEventListener('focus', () => set(true));
          host.addEventListener('blur', () => set(false));
        });
      };
      nodes.forEach((node) => {
        const id = node.dataset.id;
        const mine = edges.filter((e) => e.dataset.from === id || e.dataset.to === id);
        if (!mine.length) return;
        const near = new Set(mine.flatMap((e) => [e.dataset.from, e.dataset.to]));
        const host = node.closest('a.node-link') || node;
        // A state diagram's start and end dots have no words to announce, so they trace on hover only.
        if (host === node && textOf(node)) node.setAttribute('tabindex', '0');
        light([host], [node, ...mine, ...mine.flatMap(labels), ...nodes.filter((n) => near.has(n.dataset.id))]);
      });
      edges.forEach((e) => light([e, ...labels(e)], [e, ...labels(e), ...nodes.filter((n) => n.dataset.id === e.dataset.from || n.dataset.id === e.dataset.to)]));
    });
  }

  /** A legend entry filters its figure to that kind; pressing it again, or Escape, shows everything. */
  function legendFilters() {
    document.querySelectorAll('figure.diagram .legend').forEach((legend) => {
      const svgs = [...legend.closest('figure').querySelectorAll('svg.pele')];
      const items = [...legend.querySelectorAll('li.sem')];
      const reset = () => {
        items.forEach((i) => i.setAttribute('aria-pressed', 'false'));
        svgs.forEach((s) => { s.classList.remove('is-filtered'); s.querySelectorAll('.is-lit').forEach((e) => e.classList.remove('is-lit')); });
      };
      items.forEach((item) => {
        const kind = [...item.classList].find((c) => c.startsWith('sem-')).slice(4);
        item.setAttribute('role', 'button');
        item.setAttribute('tabindex', '0');
        item.setAttribute('aria-pressed', 'false');
        item.title = `Show only ${item.textContent.trim()}`;
        const press = () => {
          const on = item.getAttribute('aria-pressed') !== 'true';
          reset();
          if (!on) return;
          item.setAttribute('aria-pressed', 'true');
          svgs.forEach((s) => {
            s.classList.add('is-filtered');
            s.querySelectorAll(`:is(.pele-node, .pele-edge, .pele-edge-label, .pele-rect).${kind}`).forEach((e) => e.classList.add('is-lit'));
          });
        };
        item.addEventListener('click', press);
        item.addEventListener('keydown', (e) => {
          if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); press(); }
          if (e.key === 'Escape') reset();
        });
      });
    });
  }

  /** An SVG label's words: wrapped lines and code runs are separate text nodes, so they are joined with spaces. */
  const textOf = (el) => {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    const parts = [];
    for (let n = walker.nextNode(); n; n = walker.nextNode()) parts.push(n.textContent);
    return parts.join(' ').replace(/\s+/g, ' ').trim();
  };

  /**
   * A figure's tools sit in a row above its drawing — the step caption on the left, step-through then zoom on the
   * right — so they never cover a node. The row is made here, so without the script no dead control shows.
   */
  const controls = (fig, html) => {
    const frame = fig.querySelector('.diagram-frame, .free-frame');
    let wrap = frame.parentElement;
    if (!wrap.classList.contains('frame-wrap')) {
      wrap = document.createElement('div');
      wrap.className = 'frame-wrap';
      frame.before(wrap);
      wrap.append(frame);
    }
    let bar = wrap.querySelector('.fig-controls');
    if (!bar) {
      bar = document.createElement('div');
      bar.className = 'fig-controls';
      bar.setAttribute('role', 'toolbar');
      bar.setAttribute('aria-label', 'Figure tools');
      wrap.prepend(bar);
    }
    const group = document.createElement('span');
    group.className = 'fig-group';
    group.innerHTML = html;
    bar.append(group);
    return group;
  };

  /** Scrolls the drawing (and the page) just enough that `el`, the current step, is wholly in view. */
  const keepInView = (el) => {
    const f = el.closest('.diagram-frame').getBoundingClientRect();
    const r = el.getBoundingClientRect();
    if (r.left >= f.left && r.right <= f.right && r.top >= Math.max(f.top, 0) && r.bottom <= Math.min(f.bottom, innerHeight)) return;
    el.scrollIntoView({ block: 'center', inline: 'center', behavior: reduced.matches ? 'auto' : 'smooth' });
  };

  const labelsOf = (svg, id) => [...svg.querySelectorAll(`.pele-edge-label[data-id="${CSS.escape(id)}"]`)];

  /**
   * One drawing's step-through: the parts that dim, and for each step the parts drawn current, the parts kept from
   * earlier steps, the part kept in view, the caption, and the outcome the caption is coloured by.
   * @typedef {{ parts: Element[], steps: { current: Element[], kept: Element[], focus: Element, caption: string, outcome: string }[] }} StepPlan
   */

  /** @returns {StepPlan} a sequence diagram message by message, earlier messages kept */
  const messagePlan = (svg) => {
    const messages = [...svg.querySelectorAll('.pele-message')].sort((a, b) => Number(a.dataset.id.slice(1)) - Number(b.dataset.id.slice(1)));
    const withLabels = (m) => [m, ...labelsOf(svg, m.dataset.id)];
    return {
      parts: messages.flatMap(withLabels),
      steps: messages.map((m, i) => {
        const label = labelsOf(svg, m.dataset.id)[0];
        const outcome = ['failure', 'ok'].find((k) => m.classList.contains(k)) ?? '';
        return { current: withLabels(m), kept: messages.slice(0, i).flatMap(withLabels), focus: label ?? m, caption: label ? textOf(label) : '', outcome };
      }),
    };
  };

  /** @returns {StepPlan} a flowchart or state diagram in the renderer's order, the current path so far kept */
  const walkPlan = (svg, order) => {
    const nodeOf = (id) => svg.querySelector(`:is(.pele-node, .pele-cluster)[data-id="${CSS.escape(id)}"]`);
    const edgeOf = (id) => svg.querySelector(`.pele-edge[data-id="${CSS.escape(id)}"]`);
    const partsOf = (s) => [nodeOf(s.node), ...(s.edge ? [edgeOf(s.edge), ...labelsOf(svg, s.edge)] : [])].filter(Boolean);
    return {
      parts: [...svg.querySelectorAll('.pele-node, .pele-edge, .pele-edge-label')],
      steps: order.map((s, i) => {
        const path = order.slice(0, i).filter((p) => p.path === s.path);
        const opening = path[0] ?? s;
        const origin = opening.edge && edgeOf(opening.edge)?.dataset.from;
        const label = s.edge && labelsOf(svg, s.edge)[0];
        const node = nodeOf(s.node);
        return {
          current: partsOf(s),
          kept: [...path.flatMap(partsOf), ...(origin ? [nodeOf(origin)] : [])].filter(Boolean),
          focus: node,
          caption: (label && textOf(label)) || textOf(node),
          // An edge into a failure node is a failure step, as the walk itself counts it.
          outcome: !s.edge ? '' : ['failure', 'ok'].find((k) => edgeOf(s.edge)?.classList.contains(k)) ?? (node?.classList.contains('failure') ? 'failure' : ''),
        };
      }),
    };
  };

  /**
   * A sequence diagram steps message by message; a flowchart or state diagram node by node in the order the renderer
   * derived (main path, then each failure path). The current step is lit and kept in view, what came before it on its
   * path stays, the rest dims; ◂ ▸ or ←/→ move, Play runs on, Escape or All shows everything again.
   */
  function stepping() {
    document.querySelectorAll('figure[data-kind="mermaid-sequence"], figure[data-steps]').forEach((fig) => {
      const svgs = [...fig.querySelectorAll('.diagram-frame > svg.pele')];
      const plans = svgs.map((svg) => (fig.dataset.steps ? walkPlan(svg, JSON.parse(fig.dataset.steps)) : messagePlan(svg)));
      const count = plans[0].steps.length;
      if (count < 2) return;
      const noun = fig.dataset.steps ? 'step' : 'message';
      const bar = controls(fig, '<button type="button" data-do="start" aria-pressed="false">Step through</button>'
        + `<button type="button" data-do="back" hidden aria-label="Previous ${noun}">◂</button><output hidden></output>`
        + `<button type="button" data-do="next" hidden aria-label="Next ${noun}">▸</button>`
        + '<button type="button" data-do="play" hidden aria-pressed="false">Play</button>');
      const [start, back, next, play] = ['start', 'back', 'next', 'play'].map((k) => bar.querySelector(`[data-do="${k}"]`));
      const out = bar.querySelector('output');
      const caption = document.createElement('p');
      caption.className = 'step-caption';
      caption.setAttribute('aria-live', 'polite');
      caption.hidden = true;
      bar.parentElement.prepend(caption);
      let at = -1;
      let timer = null;
      const stop = () => { clearInterval(timer); timer = null; play.setAttribute('aria-pressed', 'false'); play.textContent = 'Play'; };
      const draw = () => {
        const on = at >= 0;
        plans.forEach((plan, k) => {
          const step = on ? plan.steps[at] : null;
          const lit = new Set([...(step?.current ?? []), ...(step?.kept ?? [])]);
          svgs[k].classList.toggle('is-stepping', on);
          svgs[k].querySelectorAll('.is-current').forEach((el) => el.classList.remove('is-current'));
          step?.current.forEach((el) => el.classList.add('is-current'));
          plan.parts.forEach((el) => el.classList.toggle('is-dim', on && !lit.has(el)));
        });
        [back, next, play, out].forEach((el) => { el.hidden = !on; });
        start.textContent = on ? 'All' : 'Step through';
        start.setAttribute('aria-pressed', String(on));
        out.textContent = on ? `${at + 1} / ${count}` : '';
        caption.hidden = !on;
        caption.textContent = on ? plans[0].steps[at].caption : '';
        caption.dataset.outcome = on ? plans[0].steps[at].outcome : '';
        back.disabled = at <= 0;
        next.disabled = at >= count - 1;
        const shown = svgs.findIndex((s) => s.getBoundingClientRect().width > 0);
        if (on && shown >= 0) keepInView(plans[shown].steps[at].focus);
      };
      const go = (to) => { stop(); at = Math.max(0, Math.min(count - 1, to)); draw(); };
      start.addEventListener('click', () => { stop(); at = at >= 0 ? -1 : 0; draw(); });
      back.addEventListener('click', () => go(at - 1));
      next.addEventListener('click', () => go(at + 1));
      play.addEventListener('click', () => {
        if (timer) { stop(); return; }
        if (at >= count - 1) at = 0;
        play.setAttribute('aria-pressed', 'true');
        play.textContent = 'Pause';
        draw();
        timer = setInterval(() => {
          if (at >= count - 1) { stop(); return; }
          at += 1;
          draw();
        }, reduced.matches ? 2200 : 1400);
      });
      fig.addEventListener('keydown', (e) => {
        if (at < 0) return;
        if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
          e.preventDefault();
          go(at + (e.key === 'ArrowRight' ? 1 : -1));
        }
        // In full screen Escape closes the screen first; the step-through stays where it was.
        if (e.key === 'Escape' && !fig.closest('.fig-overlay')) { stop(); at = -1; draw(); }
      });
    });
  }

  const ZOOM_STEP = 1.25;
  const EXPAND = '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 6V2h4M10 2h4v4M14 10v4h-4M6 14H2v-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  const COLLAPSE = '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 2v4H2M14 6h-4V2M10 14v-4h4M2 10h4v4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';

  /**
   * Every drawn diagram zooms: −, the level, +, Fit and Full screen above it; Ctrl/⌘ + wheel or a pinch at the
   * pointer; + − 0 keys while its tools have focus; drag to pan once it overflows; double-click back to where it
   * opened. Zoom resizes the SVG itself, so text stays vector-sharp and node links and tips keep working.
   */
  function zooming() {
    document.querySelectorAll('figure.figure-free > svg').forEach((svg) => {
      const frame = document.createElement('div');
      frame.className = 'free-frame';
      svg.before(frame);
      frame.append(svg);
    });
    document.querySelectorAll('figure.diagram .diagram-frame, figure.figure-free > .free-frame').forEach((frame) => {
      const fig = frame.closest('figure');
      const svgs = [...frame.querySelectorAll(':scope > svg')];
      const rest = new Map(svgs.map((s) => [s, s.getAttribute('style') ?? '']));
      const shown = () => svgs.find((s) => s.getBoundingClientRect().width > 0) ?? svgs[0];
      const bar = controls(fig, '<button type="button" data-z="out" aria-label="Zoom out">−</button><output aria-live="polite" aria-label="Zoom level">100%</output>'
        + '<button type="button" data-z="in" aria-label="Zoom in">+</button><button type="button" data-z="fit" aria-label="Fit to frame">Fit</button>'
        + `<button type="button" data-z="full" aria-label="Full screen">${EXPAND}</button>`);
      bar.classList.add('zoom-tools');
      bar.querySelectorAll('button').forEach((b) => { b.title = b.getAttribute('aria-label'); });
      const out = bar.querySelector('output');
      const full = bar.querySelector('[data-z="full"]');
      let scale = 1;
      let sizes = null; // each drawing's size at rest, held while it is zoomed
      let overlay = null;
      const measure = () => new Map(svgs.map((s) => {
        const r = s.getBoundingClientRect();
        const vb = s.viewBox.baseVal;
        const w = r.width || parseFloat(s.style.minWidth) || vb.width;
        return [s, { w, h: r.width ? r.height : (w * vb.height) / vb.width }];
      }));
      const fit = () => {
        sizes ??= measure();
        const { w, h } = sizes.get(shown());
        const cs = getComputedStyle(frame);
        const across = (frame.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight)) / w;
        if (!overlay) return Math.min(1, across);
        return Math.min(across, (frame.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom)) / h);
      };
      const home = () => (overlay ? fit() : 1);
      const pannable = () => frame.classList.toggle('is-pannable', frame.scrollWidth > frame.clientWidth + 1 || frame.scrollHeight > frame.clientHeight + 1);
      const restore = () => {
        rest.forEach((style, s) => s.setAttribute('style', style));
        sizes = null;
        scale = 1;
      };
      const apply = (next, cx = frame.clientWidth / 2, cy = frame.clientHeight / 2) => {
        const f = fit();
        const clamped = Math.min(Math.max(next, Math.min(1, f)), 4 * Math.max(1, f));
        const target = Math.abs(clamped - 1) < 0.01 ? 1 : clamped;
        const ratio = target / scale;
        const x = (frame.scrollLeft + cx) * ratio - cx;
        const y = (frame.scrollTop + cy) * ratio - cy;
        if (target === 1 && !overlay) restore();
        else sizes.forEach(({ w }, s) => Object.assign(s.style, { width: `${w * target}px`, minWidth: `${w * target}px`, maxWidth: 'none' }));
        scale = target;
        frame.classList.toggle('is-zoomed', scale !== 1);
        frame.scrollLeft = x;
        frame.scrollTop = y;
        out.textContent = `${Math.round(scale * 100)}%`;
        pannable();
      };

      let others = [];
      let placeholder = null;
      let before = 1;
      const onKey = (e) => { if (e.key === 'Escape') close(); };
      const open = () => {
        before = scale;
        placeholder = document.createElement('div');
        placeholder.className = 'fig-placeholder';
        placeholder.style.height = `${fig.offsetHeight}px`;
        fig.replaceWith(placeholder);
        overlay = document.createElement('div');
        overlay.className = 'fig-overlay';
        overlay.setAttribute('role', 'dialog');
        overlay.setAttribute('aria-modal', 'true');
        const cap = fig.querySelector('figcaption');
        overlay.setAttribute('aria-label', cap ? words(cap) : 'Figure');
        overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
        overlay.append(fig);
        // The rest of the page goes inert so focus stays in the figure; tips stay live, they show over it.
        others = [...document.body.children].filter((el) => !el.inert && !el.matches('.tip'));
        others.forEach((el) => { el.inert = true; });
        document.body.append(overlay);
        root.classList.add('has-overlay');
        document.addEventListener('keydown', onKey);
        full.setAttribute('aria-label', 'Exit full screen');
        full.title = 'Exit full screen (Esc)';
        full.innerHTML = COLLAPSE;
        apply(fit());
        full.focus();
      };
      const close = () => {
        document.removeEventListener('keydown', onKey);
        others.forEach((el) => { el.inert = false; });
        placeholder.replaceWith(fig);
        overlay.remove();
        overlay = null;
        root.classList.remove('has-overlay');
        full.setAttribute('aria-label', 'Full screen');
        full.title = 'Full screen';
        full.innerHTML = EXPAND;
        restore();
        apply(before);
        full.focus();
      };

      bar.addEventListener('click', (e) => {
        const z = e.target.closest('button')?.dataset.z;
        if (z === 'in') apply(scale * ZOOM_STEP);
        if (z === 'out') apply(scale / ZOOM_STEP);
        if (z === 'fit') apply(fit());
        if (z === 'full') (overlay ? close : open)();
      });
      fig.addEventListener('keydown', (e) => {
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (e.key === '+' || e.key === '=') apply(scale * ZOOM_STEP);
        if (e.key === '-') apply(scale / ZOOM_STEP);
        if (e.key === '0') apply(home());
      });
      const at = (x, y) => {
        const r = frame.getBoundingClientRect();
        return [x - r.left, y - r.top];
      };
      frame.addEventListener('wheel', (e) => {
        if (!e.ctrlKey && !e.metaKey) return;
        e.preventDefault();
        apply(scale * (e.deltaY < 0 ? 1.1 : 1 / 1.1), ...at(e.clientX, e.clientY));
      }, { passive: false });
      frame.addEventListener('dblclick', (e) => { if (!e.target.closest('a')) apply(home()); });
      // A double-click resets the zoom; it must not also select the label under it.
      frame.addEventListener('mousedown', (e) => { if (e.detail > 1) e.preventDefault(); });

      const touches = new Map();
      let pinch = null;
      const spread = () => {
        const [a, b] = [...touches.values()];
        return Math.hypot(a.x - b.x, a.y - b.y);
      };
      let drag = null;
      let dragged = false;
      frame.addEventListener('pointerdown', (e) => {
        if (e.pointerType === 'touch') {
          touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
          if (touches.size === 2) pinch = { spread: spread(), scale };
          return;
        }
        if (e.button !== 0 || !frame.classList.contains('is-pannable')) return;
        drag = { x: e.clientX, y: e.clientY, left: frame.scrollLeft, top: frame.scrollTop };
        dragged = false;
      });
      frame.addEventListener('pointermove', (e) => {
        if (!touches.has(e.pointerId)) return;
        touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (!pinch || touches.size !== 2) return;
        const [a, b] = [...touches.values()];
        apply(pinch.scale * (spread() / pinch.spread), ...at((a.x + b.x) / 2, (a.y + b.y) / 2));
      });
      ['pointerup', 'pointercancel'].forEach((type) => frame.addEventListener(type, (e) => {
        touches.delete(e.pointerId);
        if (touches.size < 2) pinch = null;
      }));
      addEventListener('pointermove', (e) => {
        if (!drag) return;
        const dx = e.clientX - drag.x;
        const dy = e.clientY - drag.y;
        if (!dragged && Math.hypot(dx, dy) < 4) return;
        dragged = true;
        frame.classList.add('is-panning');
        frame.scrollLeft = drag.left - dx;
        frame.scrollTop = drag.top - dy;
      });
      addEventListener('pointerup', () => { drag = null; frame.classList.remove('is-panning'); });
      // A drag that ends on a node link must not follow it.
      frame.addEventListener('click', (e) => { if (dragged) { e.preventDefault(); e.stopPropagation(); dragged = false; } }, true);
      pannable();
    });
  }

  /**
   * A box that scrolls sideways fades at each edge that hides content and, until its end is reached, says it scrolls,
   * so a cut-off node or column never reads as the end of the figure or table.
   */
  function scrollCues() {
    document.querySelectorAll('.scroll').forEach((box) => {
      const cue = document.createElement('div');
      cue.className = 'scroll-cue';
      cue.setAttribute('aria-hidden', 'true');
      cue.innerHTML = '<span>scroll →</span>';
      box.after(cue);
      const update = () => {
        const hidden = box.scrollWidth - box.clientWidth;
        const right = hidden > 1 && box.scrollLeft < hidden - 1;
        box.classList.toggle('has-more-left', box.scrollLeft > 1);
        box.classList.toggle('has-more-right', right);
        cue.hidden = !right;
      };
      box.addEventListener('scroll', update, { passive: true });
      new ResizeObserver(update).observe(box);
      update();
    });
  }

  document.addEventListener('DOMContentLoaded', () => {
    const theme = themeToggles();
    navigation(theme);
    theme.paint();
    tips();
    openTargets();
    snippetNotes();
    tableColumns();
    tracing();
    legendFilters();
    stepping();
    zooming();
    scrollCues();
  });
})();
