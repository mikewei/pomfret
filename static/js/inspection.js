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
  var requestsCache = [];

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
      html += '<tr><td class="insp-meta-name">' + escapeHtml(r[0]) + '</td><td class="insp-meta-value">' + escapeHtml(String(r[1])) + '</td></tr>';
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
      ['Backend', data.backend_id || '-'],
      ['Model', data.model || '-']
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
      ['Status', data.status != null ? String(data.status) : '-']
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

  function showDetail(id) {
    selectedId = id;
    listEl.querySelectorAll('.insp-row-selected').forEach(function (r) { r.classList.remove('insp-row-selected'); });
    var row = listEl.querySelector('tr[data-id="' + escapeCssAttr(id) + '"]');
    if (row) {
      row.classList.add('insp-row-selected');
      row.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
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
          return;
        }
        renderDetailSummary(data);
        renderBody(detailRequestEl, data.request_body, t('none'));
        renderBody(detailResponseEl, data.response_body, t('none'));
        detailTabsEl.querySelector('.insp-tab.active') && detailTabsEl.querySelector('.insp-tab.active').classList.remove('active');
        detailTabsEl.querySelector('[data-panel="request"]').classList.add('active');
        detailRequestEl.closest('.insp-detail-panel').classList.remove('insp-panel-hidden');
        detailResponseEl.closest('.insp-detail-panel').classList.add('insp-panel-hidden');
      })
      .catch(function () {
        detailRequestEl.innerHTML = '<p class="insp-error">' + escapeHtml(t('loadFailed')) + '</p>';
        detailResponseEl.innerHTML = '';
      });
  }

  function switchDetailTab(panelName) {
    detailTabsEl.querySelectorAll('.insp-tab').forEach(function (tab) {
      tab.classList.toggle('active', tab.getAttribute('data-panel') === panelName);
    });
    detailRequestEl.closest('.insp-detail-panel').classList.toggle('insp-panel-hidden', panelName !== 'request');
    detailResponseEl.closest('.insp-detail-panel').classList.toggle('insp-panel-hidden', panelName !== 'response');
  }

  function renderList(requests) {
    requestsCache = requests || [];
    if (!listEl) return;
    if (!requests || requests.length === 0) {
      listEl.innerHTML = '<tr><td colspan="6">' + escapeHtml(t('noRequests')) + '</td></tr>';
      detailPanelEl.hidden = true;
      selectedId = null;
      return;
    }
    var limit = 100;
    var rows = requests.slice(0, limit).map(function (r) {
      var time = new Date(r.created_at * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });
      var statusClass = r.status >= 200 && r.status < 300 ? 'status-ok' : 'status-fail';
      var statusText = r.status != null ? r.status : '-';
      var selected = r.id === selectedId ? ' insp-row-selected' : '';
      return '<tr class="insp-trace-row' + selected + '" data-id="' + escapeHtml(r.id) + '" role="button" tabindex="0">' +
        '<td class="insp-cell-time">' + escapeHtml(time) + '</td>' +
        '<td class="insp-cell-method">' + escapeHtml(r.method) + '</td>' +
        '<td class="insp-cell-path">' + escapeHtml(r.path) + '</td>' +
        '<td class="insp-cell-backend">' + escapeHtml(r.backend_id || '-') + '</td>' +
        '<td class="insp-cell-model">' + escapeHtml(r.model || '-') + '</td>' +
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
  }

  function getSelectedId() {
    return selectedId;
  }

  function refreshDetail() {
    if (selectedId) showDetail(selectedId);
  }

  global.Inspection = {
    init: init,
    setT: setT,
    loadRequests: loadRequests,
    showDetail: showDetail,
    getSelectedId: getSelectedId,
    refreshDetail: refreshDetail,
    renderList: renderList
  };
})(typeof window !== 'undefined' ? window : this);
