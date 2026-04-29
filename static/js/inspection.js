/**
 * Inspection module: trace list + detail panel (Wireshark-style).
 * Renders request/response as collapsible JSON tree or raw text.
 */
(function (global) {
  'use strict';

  var t = function (k) { return k; };
  var listEl = null;
  var detailPanelEl = null;
  var detailSummaryEl = null;
  var detailTabsEl = null;
  var detailRequestEl = null;
  var detailResponseEl = null;
  var selectedId = null;
  var activeDetailTab = 'request';
  var requestsCache = [];
  var backendsMap = {};

  /** --- Request body search (server ids + client highlights) --- */
  var MAX_SEARCH_MARKS = 400;
  var MAX_SEARCH_QUERY_LEN = 256;
  // Array "jump to last" button threshold (only show for arrays longer than this).
  var ARRAY_JUMP_LAST_MIN_LEN = 5;
  var searchNeedle = '';
  var matchedRecordIds = [];
  var searchMatchedTotal = 0;
  var searchTruncated = false;
  var recordIdx = 0;
  var occIdx = 0;
  var searchMarkEls = [];

  var dockSearchPanel = null;
  var dockToolbarSearch = null;
  var dockPanelClose = null;
  var dockInput = null;
  var dockSearchRun = null;
  var dockClearBtn = null;
  var dockStatsEl = null;
  var dockPrevRecord = null;
  var dockNextRecord = null;
  var dockPrevOcc = null;
  var dockNextOcc = null;

  function setDockPanelOpen(open) {
    if (!dockSearchPanel) return;
    dockSearchPanel.hidden = !open;
    document.body.classList.toggle('dock-panel-open', !!open);
  }

  function toggleDockSearchPanel() {
    if (!dockSearchPanel) return;
    setDockPanelOpen(dockSearchPanel.hidden);
  }

  function refreshDockAriaLabels() {
    if (dockToolbarSearch) {
      var lab = t('dockSearchTitle');
      dockToolbarSearch.setAttribute('aria-label', lab);
      dockToolbarSearch.setAttribute('title', lab);
    }
    if (dockPanelClose) {
      var closeLab = t('dockPanelClose');
      dockPanelClose.setAttribute('aria-label', closeLab);
      dockPanelClose.setAttribute('title', closeLab);
    }
    if (dockSearchRun) {
      var runLab = t('dockSearch');
      dockSearchRun.setAttribute('aria-label', runLab);
      dockSearchRun.setAttribute('title', runLab);
    }
    if (dockClearBtn) {
      var clr = t('dockClear');
      dockClearBtn.setAttribute('aria-label', clr);
      dockClearBtn.setAttribute('title', clr);
    }
    if (dockPrevRecord) {
      var x = t('dockPrevRecord');
      dockPrevRecord.setAttribute('aria-label', x);
      dockPrevRecord.setAttribute('title', x);
    }
    if (dockNextRecord) {
      var x = t('dockNextRecord');
      dockNextRecord.setAttribute('aria-label', x);
      dockNextRecord.setAttribute('title', x);
    }
    if (dockPrevOcc) {
      var x = t('dockPrevMatch');
      dockPrevOcc.setAttribute('aria-label', x);
      dockPrevOcc.setAttribute('title', x);
    }
    if (dockNextOcc) {
      var x = t('dockNextMatch');
      dockNextOcc.setAttribute('aria-label', x);
      dockNextOcc.setAttribute('title', x);
    }
  }

  function escapeRegExp(s) {
    return String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  function clearSearchMarksIn(root) {
    if (!root) return;
    root.querySelectorAll('mark.insp-search-hit').forEach(function (m) {
      var p = m.parentNode;
      if (!p) return;
      while (m.firstChild) p.insertBefore(m.firstChild, m);
      p.removeChild(m);
    });
    root.normalize();
  }

  function clearAllSearchMarks() {
    clearSearchMarksIn(detailRequestEl);
    clearSearchMarksIn(detailResponseEl);
    searchMarkEls = [];
  }

  function collectTextNodes(root, out) {
    if (!root) return;
    var child = root.firstChild;
    while (child) {
      var next = child.nextSibling;
      if (child.nodeType === Node.TEXT_NODE) {
        out.push(child);
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        var el = child;
        if (el.classList && el.classList.contains('insp-tree-path-tooltip')) {
          /* skip */
        } else if (el.matches && el.matches('button.insp-tree-copy')) {
          /* skip */
        } else {
          collectTextNodes(el, out);
        }
      }
      child = next;
    }
  }

  function splitTextNodeWithMatches(textNode, re, marksOut, maxMarks) {
    if (!textNode || !textNode.parentNode || marksOut.length >= maxMarks) return;
    var text = textNode.nodeValue;
    if (!text) return;
    var parent = textNode.parentNode;
    var frag = document.createDocumentFragment();
    var last = 0;
    var localRe = new RegExp(re.source, re.flags);
    var m;
    var any = false;
    while ((m = localRe.exec(text)) !== null) {
      if (marksOut.length >= maxMarks) break;
      any = true;
      if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
      var mk = document.createElement('mark');
      mk.className = 'insp-search-hit';
      mk.appendChild(document.createTextNode(m[0]));
      marksOut.push(mk);
      frag.appendChild(mk);
      last = m.index + m[0].length;
      if (m.index === localRe.lastIndex) localRe.lastIndex++;
    }
    if (!any) return;
    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
    parent.replaceChild(frag, textNode);
  }

  function applySearchHighlights() {
    clearAllSearchMarks();
    searchMarkEls = [];
    if (!searchNeedle || !detailRequestEl || !detailResponseEl) return;
    var esc = escapeRegExp(searchNeedle);
    var re;
    try {
      re = new RegExp(esc, 'gi');
    } catch (_) {
      return;
    }
    [detailRequestEl, detailResponseEl].forEach(function (root) {
      var nodes = [];
      collectTextNodes(root, nodes);
      nodes.forEach(function (tn) {
        if (searchMarkEls.length >= MAX_SEARCH_MARKS) return;
        if (!tn.parentNode) return;
        splitTextNodeWithMatches(tn, re, searchMarkEls, MAX_SEARCH_MARKS);
      });
    });
  }

  function updateOccHighlight() {
    if (!searchMarkEls.length) {
      updateDockStats();
      updateDockButtonState();
      return;
    }
    searchMarkEls.forEach(function (m, i) {
      m.classList.toggle('insp-search-hit-current', i === occIdx);
    });
    var cur = searchMarkEls[occIdx];
    if (cur) {
      var wrapReq = document.getElementById('insp-detail-request-wrap');
      var inReq = wrapReq && wrapReq.contains(cur);
      switchDetailTab(inReq ? 'request' : 'response');
      try {
        cur.scrollIntoView({ block: 'center', behavior: 'smooth' });
      } catch (_) {
        cur.scrollIntoView({ block: 'center' });
      }
    }
    updateDockStats();
    updateDockButtonState();
  }

  function updateDockStats() {
    if (!dockStatsEl) return;
    if (!searchNeedle && (!matchedRecordIds || !matchedRecordIds.length)) {
      dockStatsEl.textContent = '';
      return;
    }
    var lines = [];
    if (matchedRecordIds && matchedRecordIds.length) {
      var tr = searchTruncated ? t('dockTruncatedHint') : '';
      lines.push(t('dockStatsTotal') + ' ' + searchMatchedTotal + (tr ? ' (' + tr + ')' : ''));
      lines.push(t('dockStatsRecordNav') + ' ' + (recordIdx + 1) + ' / ' + matchedRecordIds.length);
    }
    if (searchMarkEls.length) {
      lines.push(t('dockStatsMatchNav') + ' ' + (occIdx + 1) + ' / ' + searchMarkEls.length);
    } else if (searchNeedle && matchedRecordIds && matchedRecordIds.length) {
      lines.push(t('dockNoMatchesInRecord'));
    }
    dockStatsEl.textContent = lines.join('\n');
  }

  function updateDockButtonState() {
    var nRec = matchedRecordIds ? matchedRecordIds.length : 0;
    var nOcc = searchMarkEls.length;
    if (dockPrevRecord) dockPrevRecord.disabled = !nRec || recordIdx <= 0;
    if (dockNextRecord) dockNextRecord.disabled = !nRec || recordIdx >= nRec - 1;
    if (dockPrevOcc) dockPrevOcc.disabled = nOcc <= 1;
    if (dockNextOcc) dockNextOcc.disabled = nOcc <= 1;
  }

  function finishSearchAfterDetail(data, options) {
    if (options.onDetailRendered) {
      try { options.onDetailRendered(data); } catch (e) {}
    }
    if (searchNeedle && data) {
      applySearchHighlights();
      if (options.preserveOccIdx) {
        if (occIdx >= searchMarkEls.length) occIdx = Math.max(0, searchMarkEls.length - 1);
      } else {
        occIdx = 0;
      }
      if (searchMarkEls.length) updateOccHighlight();
      else {
        updateDockStats();
        updateDockButtonState();
      }
    }
  }

  function runSearch() {
    if (!dockInput || !dockStatsEl) return;
    var q = (dockInput.value || '').trim();
    if (!q) {
      dockStatsEl.textContent = '';
      return;
    }
    setDockPanelOpen(true);
    if (q.length > MAX_SEARCH_QUERY_LEN) {
      dockStatsEl.textContent = t('dockQueryTooLong');
      return;
    }
    dockStatsEl.textContent = t('dockSearching');
    fetch('/api/requests/search?q=' + encodeURIComponent(q) + '&limit=200')
      .then(function (r) {
        if (r.status === 400) return Promise.reject({ badRequest: true });
        return r.json();
      })
      .then(function (res) {
        searchNeedle = q;
        matchedRecordIds = res.ids || [];
        searchMatchedTotal = res.matched_records != null ? res.matched_records : matchedRecordIds.length;
        searchTruncated = !!res.truncated;
        recordIdx = 0;
        occIdx = 0;
        if (!matchedRecordIds.length) {
          searchNeedle = '';
          clearAllSearchMarks();
          dockStatsEl.textContent = t('dockNoResults');
          updateDockButtonState();
          return;
        }
        showDetail(matchedRecordIds[0], { resetTab: true, skipAutoScroll: false });
      })
      .catch(function (err) {
        if (err && err.badRequest) dockStatsEl.textContent = t('dockQueryTooLong');
        else dockStatsEl.textContent = t('dockSearchFailed');
        updateDockButtonState();
      });
  }

  function clearSearchSession() {
    searchNeedle = '';
    matchedRecordIds = [];
    searchMatchedTotal = 0;
    searchTruncated = false;
    recordIdx = 0;
    occIdx = 0;
    clearAllSearchMarks();
    if (dockInput) dockInput.value = '';
    if (dockStatsEl) dockStatsEl.textContent = '';
    updateDockButtonState();
  }

  function goPrevRecord() {
    if (recordIdx <= 0) return;
    recordIdx--;
    showDetail(matchedRecordIds[recordIdx], { resetTab: true, skipAutoScroll: false });
  }

  function goNextRecord() {
    if (recordIdx >= matchedRecordIds.length - 1) return;
    recordIdx++;
    showDetail(matchedRecordIds[recordIdx], { resetTab: true, skipAutoScroll: false });
  }

  function goPrevOcc() {
    if (!searchMarkEls.length) return;
    occIdx = (occIdx - 1 + searchMarkEls.length) % searchMarkEls.length;
    updateOccHighlight();
  }

  function goNextOcc() {
    if (!searchMarkEls.length) return;
    occIdx = (occIdx + 1) % searchMarkEls.length;
    updateOccHighlight();
  }

  function initSearchDock() {
    dockSearchPanel = document.getElementById('dock-search-panel');
    dockToolbarSearch = document.getElementById('dock-toolbar-search');
    dockPanelClose = document.getElementById('dock-panel-close');
    dockInput = document.getElementById('dock-search-input');
    dockSearchRun = document.getElementById('dock-search-run');
    dockClearBtn = document.getElementById('dock-search-clear');
    dockStatsEl = document.getElementById('dock-search-stats');
    dockPrevRecord = document.getElementById('dock-prev-record');
    dockNextRecord = document.getElementById('dock-next-record');
    dockPrevOcc = document.getElementById('dock-prev-occ');
    dockNextOcc = document.getElementById('dock-next-occ');
    if (dockToolbarSearch) {
      dockToolbarSearch.onclick = function (e) {
        e.preventDefault();
        toggleDockSearchPanel();
      };
    }
    if (dockPanelClose) {
      dockPanelClose.onclick = function (e) {
        e.preventDefault();
        setDockPanelOpen(false);
      };
    }
    if (dockSearchRun) dockSearchRun.onclick = function () { runSearch(); };
    if (dockClearBtn) dockClearBtn.onclick = function () { clearSearchSession(); };
    if (dockPrevRecord) dockPrevRecord.onclick = function () { goPrevRecord(); };
    if (dockNextRecord) dockNextRecord.onclick = function () { goNextRecord(); };
    if (dockPrevOcc) dockPrevOcc.onclick = function () { goPrevOcc(); };
    if (dockNextOcc) dockNextOcc.onclick = function () { goNextOcc(); };
    if (dockInput) {
      dockInput.onkeydown = function (e) {
        if (e.key === 'Enter') {
          e.preventDefault();
          runSearch();
        }
      };
    }
    document.addEventListener('i18n:changed', function () {
      refreshDockAriaLabels();
      updateDockStats();
    });
    refreshDockAriaLabels();
    updateDockButtonState();
  }

  function setBackends(list) {
    backendsMap = {};
    (list || []).forEach(function (b) { backendsMap[b.id] = b.name || b.id; });
  }

  function backendLabel(id) {
    if (!id) return '-';
    return backendsMap[id] || id;
  }

  function backendModelLabel(r) {
    var name = r.backend_name || backendLabel(r.backend_id);
    var bm = r.backend_model;
    if (!name || name === '-') return '-';
    if (bm) return name + ' (' + bm + ')';
    return name;
  }

  function escapeHtml(s) {
    if (s == null) return '';
    var div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
  }

  /** Escape for use inside a CSS double-quoted attribute selector (e.g. [data-id="..."]). */
  function escapeCssAttr(val) {
    if (val == null) return '';
    return String(val).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  }

  function setT(fn) {
    t = fn || function (k) { return k; };
  }

  /** Build path string for a key under parent path (object key or array index). */
  function pathFor(parentPath, keyLabel, isArrayIndex) {
    if (isArrayIndex) return parentPath + '[' + keyLabel + ']';
    return parentPath ? parentPath + '.' + keyLabel : keyLabel;
  }

  function showPathTooltip(tooltipEl, path, ev) {
    if (!tooltipEl || !path) return;
    tooltipEl.textContent = path;
    tooltipEl.classList.add('insp-tree-path-tooltip-visible');
    var rect = ev.target.getBoundingClientRect();
    tooltipEl.style.left = rect.left + 'px';
    tooltipEl.style.top = (rect.top - 4) + 'px';
    tooltipEl.style.transform = 'translateY(-100%)';
  }

  function hidePathTooltip(tooltipEl) {
    if (tooltipEl) tooltipEl.classList.remove('insp-tree-path-tooltip-visible');
  }

  function appendKeyWithPath(lineOrHead, keyLabel, fullPath, tooltipEl) {
    var keyWrap = document.createElement('span');
    keyWrap.className = 'insp-tree-key-wrap';
    keyWrap.setAttribute('data-path', fullPath);
    var keySpan = document.createElement('span');
    keySpan.className = 'insp-tree-key';
    keySpan.textContent = keyLabel;
    keyWrap.appendChild(keySpan);
    keyWrap.onmouseenter = function (e) {
      showPathTooltip(tooltipEl, fullPath, e);
    };
    keyWrap.onmouseleave = function () {
      hidePathTooltip(tooltipEl);
    };
    lineOrHead.appendChild(keyWrap);
    lineOrHead.appendChild(document.createTextNode(': '));
  }

  function appendValueWithCopy(line, value, tooltipEl) {
    var valueWrap = document.createElement('span');
    valueWrap.className = 'insp-tree-value-wrap';
    var valSpan = document.createElement('span');
    valSpan.className = typeof value === 'string' ? 'insp-tree-string' : 'insp-tree-prim';
    valSpan.textContent = value;
    valueWrap.appendChild(valSpan);
    var copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'insp-tree-copy';
    copyBtn.setAttribute('aria-label', 'Copy');
    copyBtn.textContent = 'Copy';
    copyBtn.onclick = function (e) {
      e.preventDefault();
      e.stopPropagation();
      var text = typeof value === 'string' ? value : String(value);
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(function () {
          copyBtn.textContent = 'Copied!';
          copyBtn.blur();
          setTimeout(function () { copyBtn.textContent = 'Copy'; }, 1200);
        });
      } else {
        copyBtn.blur();
      }
    };
    valueWrap.appendChild(copyBtn);

    var floatingThreshold = 30;
    valueWrap.onmousemove = function (e) {
      var rect = valueWrap.getBoundingClientRect();
      if (rect.height > floatingThreshold) {
        var relY = e.clientY - rect.top - 9;
        relY = Math.max(0, Math.min(relY, rect.height - 18));
        copyBtn.style.position = 'absolute';
        copyBtn.style.right = '0';
        copyBtn.style.top = relY + 'px';
        copyBtn.style.marginLeft = '0';
      } else {
        copyBtn.style.position = '';
        copyBtn.style.right = '';
        copyBtn.style.top = '';
        copyBtn.style.marginLeft = '';
      }
    };
    valueWrap.onmouseleave = function () {
      copyBtn.style.position = '';
      copyBtn.style.right = '';
      copyBtn.style.top = '';
      copyBtn.style.marginLeft = '';
    };

    line.appendChild(valueWrap);
  }

  /**
   * Render JSON as collapsible tree. Returns DOM fragment.
   * path: current path in JSON (e.g. "messages[0]"); tooltipEl: element for path tooltip.
   */
  function renderJsonTree(value, keyLabel, depth, path, tooltipEl) {
    depth = depth || 0;
    path = path != null ? path : '';
    tooltipEl = tooltipEl || null;
    var indent = depth * 12;
    var wrap = document.createElement('div');
    wrap.className = 'insp-tree-node';
    wrap.style.setProperty('--depth', depth);

    var currentPath = keyLabel != null ? pathFor(path, keyLabel, typeof keyLabel === 'number') : path;

    if (value === null || value === undefined) {
      var prim = document.createElement('div');
      prim.className = 'insp-tree-line';
      prim.style.paddingLeft = indent + 'px';
      if (keyLabel != null) {
        appendKeyWithPath(prim, keyLabel, currentPath, tooltipEl);
      }
      var nullSpan = document.createElement('span');
      nullSpan.className = 'insp-tree-null';
      nullSpan.textContent = value === null ? 'null' : 'undefined';
      prim.appendChild(nullSpan);
      wrap.appendChild(prim);
      return wrap;
    }

    if (typeof value !== 'object') {
      var line = document.createElement('div');
      line.className = 'insp-tree-line';
      line.style.paddingLeft = indent + 'px';
      if (keyLabel != null) {
        appendKeyWithPath(line, keyLabel, currentPath, tooltipEl);
      }
      appendValueWithCopy(line, value, tooltipEl);
      wrap.appendChild(line);
      return wrap;
    }

    var isArray = Array.isArray(value);
    var count = isArray ? value.length : Object.keys(value).length;
    var prefix = isArray ? '[' : '{';
    var suffix = isArray ? ']' : '}';
    var head = document.createElement('div');
    head.className = 'insp-tree-toggle';
    head.style.paddingLeft = indent + 'px';
    head.setAttribute('role', 'button');
    head.setAttribute('tabindex', '0');
    head.setAttribute('aria-expanded', 'true');
    if (keyLabel != null) {
      appendKeyWithPath(head, keyLabel, currentPath, tooltipEl);
    }
    head.appendChild(document.createElement('span')).className = 'insp-tree-bracket';
    head.lastChild.textContent = prefix;
    head.appendChild(document.createTextNode(' '));
    var countSpan = document.createElement('span');
    countSpan.className = 'insp-tree-count';
    countSpan.textContent = count + (isArray ? ' items' : ' keys');
    head.appendChild(countSpan);
    head.appendChild(document.createTextNode(' '));
    head.appendChild(document.createElement('span')).className = 'insp-tree-bracket';
    head.lastChild.textContent = suffix;

    // For long arrays, offer a quick "jump to last element" hover button.
    if (isArray && value.length > ARRAY_JUMP_LAST_MIN_LEN) {
      var jumpLastBtn = document.createElement('button');
      jumpLastBtn.type = 'button';
      jumpLastBtn.className = 'insp-tree-jump-last';
      jumpLastBtn.setAttribute('aria-label', t('jumpToLast'));
      jumpLastBtn.textContent = t('jumpToLast');
      jumpLastBtn.onclick = function (e) {
        e.preventDefault();
        e.stopPropagation();

        // Ensure the array children are visible before scrolling to the anchor.
        if (body.classList.contains('insp-tree-closed')) {
          body.classList.remove('insp-tree-closed');
          head.setAttribute('aria-expanded', 'true');
          head.classList.remove('insp-tree-toggled-closed');
        }

        var lastIdx = value.length - 1;
        var lastPath = currentPath + '[' + lastIdx + ']';

        var doScroll = function () {
          var scope = head.closest('.insp-detail-panel') || document;
          var target = scope.querySelector('.insp-tree-key-wrap[data-path="' + escapeCssAttr(lastPath) + '"]');
          if (!target) return;
          try {
            target.scrollIntoView({ block: 'center', behavior: 'smooth' });
          } catch (_) {
            target.scrollIntoView({ block: 'center' });
          }
        };

        // Next frame so class removals take effect in layout.
        try {
          requestAnimationFrame(doScroll);
        } catch (_) {
          doScroll();
        }
      };
      head.appendChild(document.createTextNode(' '));
      head.appendChild(jumpLastBtn);
    }
    wrap.appendChild(head);

    var body = document.createElement('div');
    body.className = 'insp-tree-children';
    if (isArray) {
      value.forEach(function (item, i) {
        body.appendChild(renderJsonTree(item, i, depth + 1, currentPath, tooltipEl));
      });
    } else {
      Object.keys(value).forEach(function (k) {
        body.appendChild(renderJsonTree(value[k], k, depth + 1, currentPath, tooltipEl));
      });
    }
    wrap.appendChild(body);

    head.onclick = function (e) {
      e.preventDefault();
      var open = body.classList.toggle('insp-tree-closed');
      head.setAttribute('aria-expanded', open ? 'false' : 'true');
      head.classList.toggle('insp-tree-toggled-closed', open);
    };
    head.onkeydown = function (e) {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        head.click();
      }
    };

    return wrap;
  }

  /**
   * Render body (JSON or raw) into container. Prefer tree for JSON.
   */
  function renderBody(container, raw, labelNone) {
    container.innerHTML = '';
    if (raw == null || raw === '') {
      container.textContent = labelNone || t('none');
      container.classList.add('insp-raw');
      return;
    }
    var str = typeof raw === 'string' ? raw : JSON.stringify(raw);
    try {
      var parsed = JSON.parse(str);
      var pathTooltip = document.createElement('div');
      pathTooltip.className = 'insp-tree-path-tooltip';
      pathTooltip.setAttribute('role', 'tooltip');
      container.appendChild(pathTooltip);
      var tree = renderJsonTree(parsed, null, 0, '', pathTooltip);
      container.appendChild(tree);
      container.classList.remove('insp-raw');
    } catch (_) {
      var pre = document.createElement('pre');
      pre.className = 'insp-raw-pre';
      pre.textContent = str;
      container.appendChild(pre);
      container.classList.add('insp-raw');
    }
  }

  /**
   * Parse query string into key-value pairs for display.
   */
  function parseQueryString(qs) {
    if (!qs || typeof qs !== 'string') return [];
    var pairs = [];
    qs.split('&').forEach(function (pair) {
      if (!pair) return;
      var i = pair.indexOf('=');
      if (i === -1) pairs.push([decodeURIComponent(pair), '']);
      else pairs.push([decodeURIComponent(pair.slice(0, i)), decodeURIComponent(pair.slice(i + 1))]);
    });
    return pairs;
  }

  /**
   * Parse headers JSON string into array of [name, value].
   */
  function parseHeadersJson(str) {
    if (!str || typeof str !== 'string') return [];
    try {
      var obj = JSON.parse(str);
      return Object.keys(obj).map(function (k) { return [k, obj[k]]; });
    } catch (_) { return []; }
  }

  /**
   * Byte size of string (UTF-8). Falls back to length if TextEncoder unavailable.
   */
  function getByteSize(str) {
    if (str == null || typeof str !== 'string') return 0;
    try {
      return new TextEncoder().encode(str).length;
    } catch (_) {
      return str.length;
    }
  }

  function formatByteSize(n) {
    if (n >= 1048576) return (n / 1048576).toFixed(2) + ' MB';
    if (n >= 1024) return (n / 1024).toFixed(2) + ' KB';
    return n + ' B';
  }

  function renderMetaSection(titleKey, rows) {
    if (!rows || rows.length === 0) return '';
    var html = '<div class="insp-meta-section">';
    html += '<div class="insp-meta-title">' + escapeHtml(t(titleKey)) + '</div>';
    html += '<table class="insp-meta-table"><tbody>';
    rows.forEach(function (r) {
      html += '<tr><td class="insp-meta-name" title="' + escapeHtml(r[0]) + '">' + escapeHtml(r[0]) + '</td><td class="insp-meta-value" title="' + escapeHtml(String(r[1])) + '">' + escapeHtml(String(r[1])) + '</td></tr>';
    });
    html += '</tbody></table></div>';
    return html;
  }

  function renderDetailSummary(data) {
    var time = data.created_at != null ? new Date(data.created_at * 1000).toLocaleString() : '-';

    var overviewItems = [
      ['Request ID', data.id || '-'],
      ['Method', data.method || '-'],
      ['Path', data.path || '-'],
      ['Time', time],
      ['Model', data.model || '-'],
      ['Backend (Model)', backendModelLabel(data)]
    ];

    var requestRows = [];
    if (data.request_query) {
      var qsPairs = parseQueryString(data.request_query);
      if (qsPairs.length) {
        qsPairs.forEach(function (p) { requestRows.push([p[0], p[1]]); });
      } else {
        requestRows.push(['Query', data.request_query]);
      }
    }
    var reqHeaders = parseHeadersJson(data.request_headers);
    reqHeaders.forEach(function (p) { requestRows.push([p[0], p[1]]); });
    var reqBodySize = getByteSize(data.request_body);
    requestRows.push([t('inspRequestBodySize'), reqBodySize === 0 ? '-' : formatByteSize(reqBodySize)]);

    var responseRows = [
      ['Status', data.status_label || (data.status != null ? String(data.status) : '-')]
    ];
    var respHeaders = parseHeadersJson(data.response_headers);
    respHeaders.forEach(function (p) { responseRows.push([p[0], p[1]]); });
    var respBodySize = getByteSize(data.response_body);
    responseRows.push([t('inspResponseBodySize'), respBodySize === 0 ? '-' : formatByteSize(respBodySize)]);

    var overviewHtml = '<div class="insp-overview-strip">';
    overviewItems.forEach(function (p) {
      overviewHtml += '<span class="insp-overview-item"><span class="insp-overview-label">' + escapeHtml(p[0]) + '</span> <span class="insp-overview-value">' + escapeHtml(String(p[1])) + '</span></span>';
    });
    overviewHtml += '</div>';

    var html = '<div class="insp-meta-wrap">';
    html += overviewHtml;
    html += '<div class="insp-meta-columns">';
    html += '<div class="insp-meta-col insp-meta-col-request">' + renderMetaSection('inspRequest', requestRows) + '</div>';
    html += '<div class="insp-meta-col insp-meta-col-response">' + renderMetaSection('inspResponse', responseRows) + '</div>';
    html += '</div></div>';
    detailSummaryEl.innerHTML = html;
  }

  function showDetail(id, options) {
    options = options || {};
    var shouldScroll = options.skipAutoScroll !== true;
    var shouldResetTab = options.resetTab !== false;
    var shouldSyncActiveTab = options.syncActiveTab !== false;

    selectedId = id;
    if (matchedRecordIds && matchedRecordIds.length) {
      var ix = matchedRecordIds.indexOf(id);
      if (ix >= 0) recordIdx = ix;
    }
    listEl.querySelectorAll('.insp-row-selected').forEach(function (r) { r.classList.remove('insp-row-selected'); });
    var row = listEl.querySelector('tr[data-id="' + escapeCssAttr(id) + '"]');
    if (row) {
      row.classList.add('insp-row-selected');
      if (shouldScroll) row.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }

    detailPanelEl.hidden = false;
    detailRequestEl.innerHTML = '<p class="insp-loading">' + escapeHtml(t('loading')) + '</p>';
    detailResponseEl.innerHTML = '';

    fetch('/api/requests/' + encodeURIComponent(id))
      .then(function (r) { return r.json(); })
      .then(function (data) {
        if (!data) {
          detailSummaryEl.innerHTML = '<p>' + escapeHtml(t('notFound')) + '</p>';
          detailRequestEl.innerHTML = '';
          detailResponseEl.innerHTML = '';
          clearAllSearchMarks();
          return;
        }
        renderDetailSummary(data);
        renderBody(detailRequestEl, data.request_body, t('none'));
        renderBody(detailResponseEl, data.response_body, t('none'));
        if (shouldResetTab) {
          switchDetailTab('request');
        } else {
          switchDetailTab(activeDetailTab === 'response' ? 'response' : 'request');
        }
        if (shouldSyncActiveTab) {
          activeDetailTab = detailResponseEl.closest('.insp-detail-panel').classList.contains('insp-panel-hidden') ? 'request' : 'response';
        }
        finishSearchAfterDetail(data, options);
      })
      .catch(function () {
        detailRequestEl.innerHTML = '<p class="insp-error">' + escapeHtml(t('loadFailed')) + '</p>';
        detailResponseEl.innerHTML = '';
        clearAllSearchMarks();
      });
  }

  function switchDetailTab(panelName) {
    activeDetailTab = panelName === 'response' ? 'response' : 'request';
    detailTabsEl.querySelectorAll('.insp-tab').forEach(function (tab) {
      tab.classList.toggle('active', tab.getAttribute('data-panel') === activeDetailTab);
    });
    detailRequestEl.closest('.insp-detail-panel').classList.toggle('insp-panel-hidden', activeDetailTab !== 'request');
    detailResponseEl.closest('.insp-detail-panel').classList.toggle('insp-panel-hidden', activeDetailTab !== 'response');
  }

  function getRequestStatusById(list, id) {
    if (!id || !list || !list.length) return undefined;
    var item = list.find(function (r) { return r.id === id; });
    if (!item) return undefined;
    return (item.status != null ? String(item.status) : '-') + '|' + (item.status_label || '');
  }

  function renderList(requests) {
    var prevRequests = requestsCache || [];
    var nextRequests = requests || [];
    var prevSelectedStatus = getRequestStatusById(prevRequests, selectedId);
    var nextSelectedStatus = getRequestStatusById(nextRequests, selectedId);
    requestsCache = nextRequests;
    if (!listEl) return;
    if (!requests || requests.length === 0) {
      listEl.innerHTML = '<tr><td colspan="6">' + escapeHtml(t('noRequests')) + '</td></tr>';
      detailPanelEl.hidden = true;
      selectedId = null;
      clearAllSearchMarks();
      return;
    }
    var limit = 100;
    var rows = requests.slice(0, limit).map(function (r) {
      var time = new Date(r.created_at * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });
      var statusClass = r.status >= 200 && r.status < 300 ? 'status-ok' : 'status-fail';
      var statusText = r.status_label || (r.status != null ? String(r.status) : '-');
      var selected = r.id === selectedId ? ' insp-row-selected' : '';
      return '<tr class="insp-trace-row' + selected + '" data-id="' + escapeHtml(r.id) + '" role="button" tabindex="0">' +
        '<td class="insp-cell-time">' + escapeHtml(time) + '</td>' +
        '<td class="insp-cell-method">' + escapeHtml(r.method) + '</td>' +
        '<td class="insp-cell-path" title="' + escapeHtml(r.path) + '">' + escapeHtml(r.path) + '</td>' +
        '<td class="insp-cell-model" title="' + escapeHtml(r.model || '-') + '">' + escapeHtml(r.model || '-') + '</td>' +
        '<td class="insp-cell-backend" title="' + escapeHtml(backendModelLabel(r)) + '">' + escapeHtml(backendModelLabel(r)) + '</td>' +
        '<td class="insp-cell-status ' + statusClass + '">' + statusText + '</td>' +
        '</tr>';
    }).join('');
    listEl.innerHTML = rows;

    listEl.querySelectorAll('.insp-trace-row').forEach(function (tr) {
      tr.onclick = function () {
        showDetail(tr.getAttribute('data-id'));
      };
      tr.onkeydown = function (e) {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          showDetail(tr.getAttribute('data-id'));
        }
      };
    });

    if (selectedId && !requests.some(function (r) { return r.id === selectedId; })) {
      selectedId = null;
      detailPanelEl.hidden = true;
    } else if (selectedId) {
      var row = listEl.querySelector('tr[data-id="' + escapeCssAttr(selectedId) + '"]');
      if (row) row.classList.add('insp-row-selected');
      detailPanelEl.hidden = false;

      // Refresh detail whenever selected-record status display changes.
      if (prevSelectedStatus !== undefined && nextSelectedStatus !== undefined && prevSelectedStatus !== nextSelectedStatus) {
        refreshDetail();
      }
    }
  }

  function loadRequests() {
    fetch('/api/requests')
      .then(function (r) { return r.json(); })
      .then(function (list) {
        renderList(list);
      })
      .catch(function () {
        renderList([]);
        if (listEl) listEl.innerHTML = '<tr><td colspan="6">' + escapeHtml(t('loadFailed')) + '</td></tr>';
      });
  }

  /**
   * Initialize inspection module. opts: { listSelector, detailPanelSelector, ... }
   */
  function init(opts) {
    opts = opts || {};
    listEl = document.querySelector(opts.listSelector || '#insp-trace-tbody');
    detailPanelEl = document.querySelector(opts.detailPanelSelector || '#insp-detail-panel');
    detailSummaryEl = document.querySelector(opts.detailSummarySelector || '#insp-detail-summary');
    detailTabsEl = document.querySelector(opts.detailTabsSelector || '#insp-detail-tabs');
    detailRequestEl = document.querySelector(opts.detailRequestSelector || '#insp-detail-request');
    detailResponseEl = document.querySelector(opts.detailResponseSelector || '#insp-detail-response');

    if (detailTabsEl) {
      detailTabsEl.querySelectorAll('.insp-tab').forEach(function (tab) {
        tab.onclick = function () {
          switchDetailTab(tab.getAttribute('data-panel'));
        };
      });
    }

    if (opts.t) setT(opts.t);
    initSearchDock();
  }

  function getSelectedId() {
    return selectedId;
  }

  function refreshDetail() {
    if (selectedId) {
      showDetail(selectedId, {
        skipAutoScroll: true,
        resetTab: false,
        syncActiveTab: false,
        preserveOccIdx: true
      });
    }
  }

  global.Inspection = {
    init: init,
    setT: setT,
    setBackends: setBackends,
    loadRequests: loadRequests,
    showDetail: showDetail,
    getSelectedId: getSelectedId,
    refreshDetail: refreshDetail,
    renderList: renderList
  };
})(typeof window !== 'undefined' ? window : this);
