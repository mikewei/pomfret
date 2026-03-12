(function () {
  var FRONTEND_VERSION = '0.1.0';
  var t = window.i18n && window.i18n.t ? window.i18n.t : function (k) { return k; };

  var _toastTimer;
  function showToast(msg) {
    var el = document.getElementById('toast');
    if (!el) {
      el = document.createElement('div');
      el.id = 'toast';
      el.className = 'toast';
      document.body.appendChild(el);
    }
    clearTimeout(_toastTimer);
    el.textContent = msg;
    el.classList.remove('toast-hide');
    el.classList.add('toast-visible');
    _toastTimer = setTimeout(function () {
      el.classList.add('toast-hide');
      el.classList.remove('toast-visible');
    }, 1500);
  }
  const gatewayUrlEl = document.getElementById('gateway-url');
  const curlChatEl = document.getElementById('curl-chat');
  const curlModelsEl = document.getElementById('curl-models');
  const backendsListEl = document.getElementById('backends-list');
  const clientTotalEl = document.getElementById('client-total');
  const backendStatusEl = document.getElementById('backend-status');
  const refreshStatusBtn = document.getElementById('refresh-status');
  const tabButtons = document.querySelectorAll('.tab[data-tab]');
  const tabPanels = document.querySelectorAll('.tab-panel');
  const btnExportConfig = document.getElementById('btn-export-config');

  function baseUrl() {
    return window.location.origin;
  }

  function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s == null ? '' : s;
    return div.innerHTML;
  }

  function initConnectionSection() {
    const base = baseUrl();
    const baseV1 = base + '/v1';
    gatewayUrlEl.value = baseV1;
    document.getElementById('copy-url').onclick = function () {
      gatewayUrlEl.select();
      document.execCommand('copy');
    };
    const chatCurl = 'curl -X POST "' + base + '/v1/chat/completions" \\\n  -H "Content-Type: application/json" \\\n  -d \'{"model":"gpt-3.5-turbo","messages":[{"role":"user","content":"Hello"}]}\'';
    const modelsCurl = 'curl "' + base + '/v1/models"';
    curlChatEl.querySelector('code').textContent = chatCurl;
    curlModelsEl.querySelector('code').textContent = modelsCurl;
    document.querySelectorAll('.copy-code').forEach(function (btn) {
      btn.onclick = function () {
        var target = document.querySelector('#' + btn.getAttribute('data-target') + ' code');
        if (target) {
          navigator.clipboard.writeText(target.textContent);
        }
      };
    });

    function buildOpenclawConfigJson() {
      var base = baseUrl() + '/v1';
      return JSON.stringify({
        pomfret: {
          baseUrl: base,
          apiKey: 'anything',
          api: 'openai-completions',
          authHeader: false,
          models: [
            {
              id: 'qwen3.5:9b',
              name: 'qwen3.5:9b',
              api: 'openai-completions',
              reasoning: true,
              input: ['text'],
              cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
              contextWindow: 65536,
              maxTokens: 65536
            }
          ]
        }
      }, null, 2);
    }
    var openclawModal = document.getElementById('openclaw-modal');
    var openclawContentEl = document.getElementById('openclaw-config-content');
    var btnOpenclawConfig = document.getElementById('btn-openclaw-config');
    var openclawCopyBtn = document.getElementById('openclaw-config-copy');
    var openclawCloseBtn = document.getElementById('openclaw-modal-close');
    if (btnOpenclawConfig) {
      btnOpenclawConfig.onclick = function () {
        if (openclawContentEl) openclawContentEl.querySelector('code').textContent = buildOpenclawConfigJson();
        if (openclawModal) openclawModal.hidden = false;
      };
    }
    if (openclawCopyBtn) {
      openclawCopyBtn.onclick = function () {
        var code = openclawContentEl && openclawContentEl.querySelector('code');
        if (code && navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(code.textContent);
        }
      };
    }
    if (openclawCloseBtn) openclawCloseBtn.onclick = function () { if (openclawModal) openclawModal.hidden = true; };
    if (openclawModal) {
      openclawModal.onclick = function (e) {
        if (e.target === openclawModal) openclawModal.hidden = true;
      };
    }
  }

  function renderBackendsList(backends) {
    var list = backends || [];
    var expandedIndices = {};
    var addNewVisible = false;
    if (backendsListEl) {
      backendsListEl.querySelectorAll('.backend-row.expanded').forEach(function (r) {
        var idx = r.getAttribute('data-index');
        if (idx != null) expandedIndices[idx] = true;
      });
      var editNew = document.getElementById('backend-edit-new');
      if (editNew) addNewVisible = !editNew.hidden;
    }
    var html = '';
    if (list.length === 0) {
      html += '<p class="backends-empty">' + escapeHtml(t('noBackends')) + '</p>';
    }
    list.forEach(function (b, i) {
      var typeLabel = (b.backend_type === 'ollama') ? t('backendTypeOllama') : t('backendTypeOpenAiCompat');
      html += '<div class="backend-row' + (b.is_current ? ' current' : '') + '" data-index="' + i + '">';
      html += '<div class="backend-row-head" data-index="' + i + '">';
      html += '<span class="backend-row-name-wrap">';
      html += '<span class="backend-row-name">' + escapeHtml(b.name) + '</span>';
      if (b.is_current) {
        html += '<span class="current-badge">' + escapeHtml(t('current')) + '</span>';
      }
      html += '</span>';
      html += '<span class="backend-row-type">' + escapeHtml(typeLabel) + '</span>';
      html += '<span class="backend-row-chevron" aria-hidden="true"></span>';
      html += '</div>';
      html += '<div class="backend-edit" data-index="' + i + '">';
      html += '<div class="backend-fields">';
      html += '<div><label>' + escapeHtml(t('name')) + '</label><input type="text" class="be-name" value="' + escapeHtml(b.name) + '" /></div>';
      html += '<div><label>' + escapeHtml(t('backendType')) + '</label><select class="be-backend-type"><option value="ollama"' + (b.backend_type === 'ollama' ? ' selected' : '') + '>' + escapeHtml(t('backendTypeOllama')) + '</option><option value="openai_compat"' + (b.backend_type === 'openai_compat' ? ' selected' : '') + '>' + escapeHtml(t('backendTypeOpenAiCompat')) + '</option></select></div>';
      html += '<div><label>' + escapeHtml(t('baseUrl')) + '</label><input type="text" class="be-base-url" value="' + escapeHtml(b.base_url) + '" placeholder="https://api.openai.com" /></div>';
      html += '<div><label>' + escapeHtml(t('apiKeyLabel')) + '</label><input type="password" class="be-api-key" placeholder="' + escapeHtml(b.api_key_set ? t('apiKeySet') : t('apiKeyNotSet')) + '" autocomplete="off" /></div>';
      html += '<div><label>' + escapeHtml(t('specifiedModel')) + '</label><input type="text" class="be-model" value="' + escapeHtml(b.model || '') + '" placeholder="' + escapeHtml(t('specifiedModelPlaceholder')) + '" /></div>';
      html += '</div>';
      html += '<div class="backend-actions">';
      if (!b.is_current) {
        html += '<button type="button" class="btn btn-secondary btn-small btn-set-current" data-index="' + i + '">' + escapeHtml(t('use')) + '</button>';
      }
      html += '<button type="button" class="btn btn-small btn-secondary btn-save-backend" data-index="' + i + '">' + escapeHtml(t('save')) + '</button>';
      html += '<button type="button" class="btn btn-small btn-danger btn-delete-backend" data-index="' + i + '">' + escapeHtml(t('delete')) + '</button>';
      html += '</div></div>';
      html += '</div>';
    });
    html += '<div class="backend-add-row">';
    html += '<button type="button" class="btn btn-secondary btn-add-backend" id="btn-add-backend">' + escapeHtml(t('addBackend')) + '</button>';
    html += '<div class="backend-edit backend-edit-new" id="backend-edit-new" hidden>';
    html += '<div class="backend-fields">';
    html += '<div><label>' + escapeHtml(t('name')) + '</label><input type="text" class="be-name" id="new-be-name" placeholder="" /></div>';
    html += '<div><label>' + escapeHtml(t('backendType')) + '</label><select class="be-backend-type" id="new-be-backend-type"><option value="ollama">' + escapeHtml(t('backendTypeOllama')) + '</option><option value="openai_compat" selected>' + escapeHtml(t('backendTypeOpenAiCompat')) + '</option></select></div>';
    html += '<div><label>' + escapeHtml(t('baseUrl')) + '</label><input type="text" class="be-base-url" id="new-be-base-url" placeholder="https://api.openai.com" /></div>';
    html += '<div><label>' + escapeHtml(t('apiKeyLabel')) + '</label><input type="password" class="be-api-key" id="new-be-api-key" placeholder="' + escapeHtml(t('apiKeyNotSet')) + '" autocomplete="off" /></div>';
    html += '<div><label>' + escapeHtml(t('specifiedModel')) + '</label><input type="text" class="be-model" id="new-be-model" placeholder="' + escapeHtml(t('specifiedModelPlaceholder')) + '" /></div>';
    html += '</div>';
    html += '<div class="backend-actions">';
    html += '<button type="button" class="btn btn-small btn-secondary btn-save-new-backend" id="btn-save-new-backend">' + escapeHtml(t('save')) + '</button>';
    html += '</div></div>';
    html += '</div>';
    backendsListEl.innerHTML = html;

    backendsListEl.querySelectorAll('.backend-row-head').forEach(function (head) {
      head.onclick = function () {
        var index = head.getAttribute('data-index');
        var row = backendsListEl.querySelector('.backend-row[data-index="' + index + '"]');
        var editNew = document.getElementById('backend-edit-new');
        if (editNew) editNew.hidden = true;
        var wasExpanded = row && row.classList.contains('expanded');
        backendsListEl.querySelectorAll('.backend-row').forEach(function (r) {
          r.classList.remove('expanded');
        });
        if (row && !wasExpanded) row.classList.add('expanded');
      };
    });

    var btnAdd = document.getElementById('btn-add-backend');
    if (btnAdd) {
      btnAdd.onclick = function () {
        backendsListEl.querySelectorAll('.backend-row').forEach(function (r) {
          r.classList.remove('expanded');
        });
        var editNew = document.getElementById('backend-edit-new');
        if (editNew) {
          editNew.hidden = !editNew.hidden;
          if (!editNew.hidden) {
            document.getElementById('new-be-name').value = '';
            document.getElementById('new-be-base-url').value = '';
            document.getElementById('new-be-api-key').value = '';
            document.getElementById('new-be-model').value = '';
          }
        }
      };
    }

    var btnSaveNew = document.getElementById('btn-save-new-backend');
    if (btnSaveNew) {
      btnSaveNew.onclick = function () {
        var name = (document.getElementById('new-be-name') || {}).value.trim();
        var baseUrl = (document.getElementById('new-be-base-url') || {}).value.trim();
        var apiKey = (document.getElementById('new-be-api-key') || {}).value;
        var modelVal = (document.getElementById('new-be-model') || {}).value.trim();
        var backendTypeEl = document.getElementById('new-be-backend-type');
        var backendType = backendTypeEl ? backendTypeEl.value : 'openai_compat';
        if (!name || !baseUrl) return;
        var body = { name: name, base_url: baseUrl, backend_type: backendType };
        if (apiKey !== '') body.api_key = apiKey;
        if (modelVal !== '') body.model = modelVal;
        fetch('/api/backends', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (r) { return r.json(); }).then(function (res) {
          if (res.ok) {
            saveConfigToFile().then(function () {
              showToast(t('backendSaved'));
              var editNew = document.getElementById('backend-edit-new');
              if (editNew) editNew.hidden = true;
              loadBackendsAndStatus(true);
            });
          }
        });
      };
    }

    backendsListEl.querySelectorAll('.btn-set-current').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        var index = parseInt(btn.getAttribute('data-index'), 10);
        fetch('/api/backends/current', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ index: index })
        }).then(function (r) { return r.json(); }).then(function (res) {
          if (res.ok) loadBackendsAndStatus(true);
        });
      };
    });
    backendsListEl.querySelectorAll('.btn-save-backend').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        var index = parseInt(btn.getAttribute('data-index'), 10);
        var row = backendsListEl.querySelector('.backend-row[data-index="' + index + '"]');
        if (!row) return;
        var name = row.querySelector('.be-name').value.trim();
        var baseUrl = row.querySelector('.be-base-url').value.trim();
        var apiKey = row.querySelector('.be-api-key').value;
        var modelEl = row.querySelector('.be-model');
        var modelVal = modelEl ? modelEl.value.trim() : undefined;
        var backendTypeEl = row.querySelector('.be-backend-type');
        var backendType = backendTypeEl ? backendTypeEl.value : undefined;
        var body = { name: name || undefined, base_url: baseUrl || undefined };
        if (backendType) body.backend_type = backendType;
        if (apiKey !== '') body.api_key = apiKey;
        if (modelVal !== undefined) body.model = modelVal;
        fetch('/api/backends/' + index, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (r) { return r.json(); }).then(function (res) {
          if (res.ok) {
            saveConfigToFile().then(function () {
              showToast(t('backendSaved'));
              loadBackendsAndStatus(true);
            });
          }
        });
      };
    });
    backendsListEl.querySelectorAll('.btn-delete-backend').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        var index = parseInt(btn.getAttribute('data-index'), 10);
        if (!confirm(t('confirmDeleteBackend'))) return;
        fetch('/api/backends/' + index, { method: 'DELETE' })
          .then(function (r) { return r.json(); })
          .then(function (res) {
            if (res.ok) saveConfigToFile().then(function () { loadBackendsAndStatus(true); });
          });
      };
    });

    Object.keys(expandedIndices).forEach(function (idx) {
      var row = backendsListEl.querySelector('.backend-row[data-index="' + idx + '"]');
      if (row) row.classList.add('expanded');
    });
    var editNew = document.getElementById('backend-edit-new');
    if (editNew) editNew.hidden = !addNewVisible;
  }

  function renderClientRequests(requests) {
    fetch('/api/stats').then(function (r) { return r.json(); }).then(function (s) {
      clientTotalEl.textContent = s.total_requests != null ? s.total_requests : (requests ? requests.length : 0);
    }).catch(function () {
      clientTotalEl.textContent = requests ? requests.length : 0;
    });
    if (window.Inspection) {
      window.Inspection.renderList(requests || []);
    }
  }

  function renderBackendStatus(statusList) {
    if (!statusList || statusList.length === 0) {
      backendStatusEl.innerHTML = '<tr><td colspan="5">' + escapeHtml(t('noBackendsRow')) + '</td></tr>';
      return;
    }
    var rows = statusList.map(function (s) {
      var currentBadge = s.is_current ? '<span class="badge current">' + escapeHtml(t('current')) + '</span>' : '';
      var reachBadge = s.reachable ? '<span class="badge live">' + escapeHtml(t('reachableBadge')) + '</span>' : '<span class="badge down">' + escapeHtml(t('unreachableBadge')) + '</span>';
      var lastAt = s.last_request_at ? new Date(s.last_request_at * 1000).toLocaleString() : '-';
      var err = s.last_error ? (' title="' + escapeHtml(s.last_error) + '"') : '';
      return '<tr><td>' + escapeHtml(s.name) + '</td><td>' + currentBadge + '</td><td' + err + '>' + reachBadge + '</td><td>' + s.request_count + '</td><td>' + lastAt + '</td></tr>';
    }).join('');
    backendStatusEl.innerHTML = rows;
  }

  function saveConfigToFile() {
    return fetch('/api/config/save', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'overwrite' })
    }).then(function (r) { return r.json(); }).then(function (res) {
      if (!res.ok) return Promise.reject(new Error(res.error || 'save failed'));
      return res;
    });
  }

  function isUserEditingBackends() {
    if (!backendsListEl) return false;
    var active = document.activeElement;
    if (active && backendsListEl.contains(active) &&
        (active.tagName === 'INPUT' || active.tagName === 'SELECT' || active.tagName === 'TEXTAREA')) {
      return true;
    }
    return false;
  }

  function loadBackendsAndStatus(force) {
    if (!force && isUserEditingBackends()) return;
    fetch('/api/backends').then(function (r) { return r.json(); }).then(function (list) {
      if (!force && isUserEditingBackends()) return;
      renderBackendsList(list);
    }).catch(function () {
      showConnectionLost();
      backendsListEl.innerHTML = '<p class="card-desc">' + escapeHtml(t('loadFailed')) + '</p>';
    });
    fetch('/api/backends/status').then(function (r) { return r.json(); }).then(function (list) {
      renderBackendStatus(list);
    }).catch(function () {
      showConnectionLost();
      backendStatusEl.innerHTML = '<tr><td colspan="5">' + escapeHtml(t('loadFailed')) + '</td></tr>';
    });
  }

  function loadRequests() {
    fetch('/api/requests').then(function (r) { return r.json(); }).then(function (list) {
      renderClientRequests(list);
    }).catch(function () {
      showConnectionLost();
      clientTotalEl.textContent = '-';
      if (window.Inspection) window.Inspection.renderList([]);
    });
  }

  function switchTab(tabName) {
    tabButtons.forEach(function (btn) {
      var isActive = btn.getAttribute('data-tab') === tabName;
      btn.classList.toggle('active', isActive);
      btn.setAttribute('aria-selected', isActive ? 'true' : 'false');
    });
    tabPanels.forEach(function (panel) {
      var isActive = panel.id === 'panel-' + tabName;
      panel.classList.toggle('active', isActive);
      panel.hidden = !isActive;
    });
  }

  tabButtons.forEach(function (btn) {
    btn.onclick = function () {
      switchTab(btn.getAttribute('data-tab'));
    };
  });

  function showRequestDetail(id) {
    switchTab('inspection');
    if (window.Inspection) window.Inspection.showDetail(id);
  }

  refreshStatusBtn.onclick = function () {
    refreshStatusBtn.disabled = true;
    fetch('/api/backends/status').then(function (r) { return r.json(); }).then(function (list) {
      renderBackendStatus(list);
    }).finally(function () {
      refreshStatusBtn.disabled = false;
    });
  };


  function showConfigMessage(msg, isError) {
    alert(msg);
  }
  if (btnExportConfig) {
    btnExportConfig.onclick = function () {
      fetch('/api/config/export').then(function (r) {
        if (!r.ok) return r.json().then(function (j) { throw new Error(j.error || r.statusText); });
        return r.blob();
      }).then(function (blob) {
        var a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = 'pomfret.conf';
        a.click();
        URL.revokeObjectURL(a.href);
      }).catch(function (err) {
        showConfigMessage(t('configSaveFailed') + ': ' + (err && err.message ? err.message : String(err)), true);
      });
    };
  }

  document.addEventListener('i18n:changed', function () {
    t = window.i18n && window.i18n.t ? window.i18n.t : function (k) { return k; };
    if (window.Inspection) window.Inspection.setT(t);
    if (connectionBannerEl && !connectionBannerEl.hidden) {
      if (connectionBannerTextEl) connectionBannerTextEl.textContent = t('connectionLost');
      if (connectionBannerRetryEl) connectionBannerRetryEl.textContent = t('connectionRetry');
    }
    loadBackendsAndStatus(true);
    loadRequests();
    renderVersionInfo();
    if (window.Inspection && window.Inspection.getSelectedId()) window.Inspection.refreshDetail();
  });

  var NOTIFY_RETRY_MS = 3000;
  var FALLBACK_POLL_INTERVAL_MS = 10000;

  var connectionBannerEl = document.getElementById('connection-banner');
  var connectionBannerTextEl = document.getElementById('connection-banner-text');
  var connectionBannerRetryEl = document.getElementById('connection-banner-retry');

  function showConnectionLost() {
    if (!connectionBannerEl) return;
    if (connectionBannerTextEl) connectionBannerTextEl.textContent = t('connectionLost');
    if (connectionBannerRetryEl) connectionBannerRetryEl.textContent = t('connectionRetry');
    connectionBannerEl.hidden = false;
  }

  function hideConnectionBanner() {
    if (connectionBannerEl) connectionBannerEl.hidden = true;
  }

  function tryReconnect() {
    fetch('/api/backends/status', { method: 'GET' })
      .then(function (r) { return r.ok ? r.json() : Promise.reject(); })
      .then(function () {
        hideConnectionBanner();
        loadBackendsAndStatus(true);
        loadRequests();
      })
      .catch(function () {});
  }

  if (connectionBannerRetryEl) {
    connectionBannerRetryEl.onclick = function () {
      tryReconnect();
    };
  }

  function startNotifyPoll() {
    fetch('/api/notify?timeout=25')
      .then(function (r) {
        if (!r.ok) throw new Error(r.statusText);
        return r.json();
      })
      .then(function (body) {
        hideConnectionBanner();
        var events = body.events || [];
        if (events.indexOf('requests') !== -1) {
          loadRequests();
          if (window.Inspection && window.Inspection.getSelectedId()) window.Inspection.refreshDetail();
        }
        if (events.indexOf('backends') !== -1) loadBackendsAndStatus();
        setTimeout(startNotifyPoll, 0);
      })
      .catch(function () {
        showConnectionLost();
        setTimeout(startNotifyPoll, NOTIFY_RETRY_MS);
      });
  }

  var backendVersion = null;
  var versionInfoEl = document.getElementById('version-info');

  function renderVersionInfo() {
    if (!versionInfoEl) return;
    var bv = backendVersion || t('versionUnknown');
    versionInfoEl.textContent = t('versionFrontend') + ' v' + FRONTEND_VERSION + ' · ' + t('versionBackend') + ' v' + bv;
  }

  function loadBackendVersion() {
    fetch('/api/version')
      .then(function (r) { return r.json(); })
      .then(function (data) {
        backendVersion = data.version || null;
        renderVersionInfo();
      })
      .catch(function () {
        backendVersion = null;
        renderVersionInfo();
      });
  }

  function init() {
    initConnectionSection();
    if (window.Inspection) window.Inspection.init({ t: t });
    loadBackendsAndStatus(true);
    loadRequests();
    loadBackendVersion();
    renderVersionInfo();
    startNotifyPoll();
    setInterval(function () {
      loadRequests();
      loadBackendsAndStatus();
    }, FALLBACK_POLL_INTERVAL_MS);
  }

  init();
})();
