(function () {
  var FRONTEND_VERSION = window.__POMFRET_FRONTEND_VERSION__ || '0.1.0';
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

  var _confirmModalResolve = null;
  function initConfirmModal() {
    var modal = document.getElementById('confirm-modal');
    if (!modal) return;
    var descEl = document.getElementById('confirm-modal-desc');
    var okBtn = document.getElementById('confirm-modal-ok');
    var cancelBtn = document.getElementById('confirm-modal-cancel');

    function closeWith(val) {
      modal.hidden = true;
      var r = _confirmModalResolve;
      _confirmModalResolve = null;
      if (r) r(val);
    }

    if (okBtn) okBtn.onclick = function () { closeWith(true); };
    if (cancelBtn) cancelBtn.onclick = function () { closeWith(false); };
    modal.onclick = function (e) {
      if (e.target === modal) closeWith(false);
    };
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && modal && !modal.hidden) closeWith(false);
    });
    if (descEl) descEl.textContent = '';
  }

  function confirmModal(descText) {
    var modal = document.getElementById('confirm-modal');
    var descEl = document.getElementById('confirm-modal-desc');
    if (!modal) return Promise.resolve(confirm(descText));
    if (descEl) descEl.textContent = descText || '';
    modal.hidden = false;
    return new Promise(function (resolve) {
      _confirmModalResolve = resolve;
    });
  }
  const gatewayUrlEl = document.getElementById('gateway-url');
  const curlChatEl = document.getElementById('curl-chat');
  const curlModelsEl = document.getElementById('curl-models');
  const backendsListEl = document.getElementById('backends-list');
  const clientTotalEl = document.getElementById('client-total');
  const backendStatusEl = document.getElementById('backend-status');
  const tabButtons = document.querySelectorAll('.tab[data-tab]');
  const tabPanels = document.querySelectorAll('.tab-panel');
  const btnExportConfig = document.getElementById('btn-export-config');
  initConfirmModal();

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
      var typeLabel = (b.backend_type === 'ollama') ? t('backendTypeOllama') : (b.backend_type === 'gemini') ? t('backendTypeGemini') : t('backendTypeOpenAiCompat');
      html += '<div class="backend-row" data-index="' + i + '">';
      html += '<div class="backend-row-head" data-index="' + i + '">';
      html += '<span class="backend-row-name-wrap">';
      html += '<span class="backend-row-name">' + escapeHtml(b.name) + '</span>';
      html += '</span>';
      html += '<span class="backend-row-type">' + escapeHtml(typeLabel) + '</span>';
      html += '<span class="backend-row-chevron" aria-hidden="true"></span>';
      html += '</div>';
      html += '<div class="backend-edit" data-index="' + i + '">';
      html += '<div class="backend-fields">';
      html += '<div><label>' + escapeHtml(t('name')) + '</label><input type="text" class="be-name" value="' + escapeHtml(b.name) + '" /></div>';
      html += '<div><label>' + escapeHtml(t('backendType')) + '</label><select class="be-backend-type"><option value="ollama"' + (b.backend_type === 'ollama' ? ' selected' : '') + '>' + escapeHtml(t('backendTypeOllama')) + '</option><option value="openai_compat"' + (b.backend_type === 'openai_compat' ? ' selected' : '') + '>' + escapeHtml(t('backendTypeOpenAiCompat')) + '</option><option value="gemini"' + (b.backend_type === 'gemini' ? ' selected' : '') + '>' + escapeHtml(t('backendTypeGemini')) + '</option></select></div>';
      html += '<div><label>' + escapeHtml(t('baseUrl')) + '</label><input type="text" class="be-base-url" value="' + escapeHtml(b.base_url) + '" placeholder="https://api.openai.com" /></div>';
      var apiKeyPlaceholder = (b.api_key_set ? t('apiKeySet') : t('apiKeyNotSet'));
      if (b.api_key_set && b.api_key_hint) apiKeyPlaceholder += ' ' + b.api_key_hint;
      html += '<div><label>' + escapeHtml(t('apiKeyLabel')) + '</label><input type="password" class="be-api-key" placeholder="' + escapeHtml(apiKeyPlaceholder) + '" autocomplete="off" /></div>';
      html += '<div><label>' + escapeHtml(t('specifiedModel')) + '</label><input type="text" class="be-model" value="' + escapeHtml(b.model || '') + '" placeholder="' + escapeHtml(t('specifiedModelPlaceholder')) + '" /></div>';
      html += '</div>';
      html += '<div class="backend-actions">';
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
    html += '<div><label>' + escapeHtml(t('backendType')) + '</label><select class="be-backend-type" id="new-be-backend-type"><option value="ollama">' + escapeHtml(t('backendTypeOllama')) + '</option><option value="openai_compat" selected>' + escapeHtml(t('backendTypeOpenAiCompat')) + '</option><option value="gemini">' + escapeHtml(t('backendTypeGemini')) + '</option></select></div>';
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

  var statTotalTokensEl = document.getElementById('stat-total-tokens');
  var statPromptTokensEl = document.getElementById('stat-prompt-tokens');
  var statCompletionTokensEl = document.getElementById('stat-completion-tokens');

  function formatNumber(n) {
    if (n == null) return '0';
    return Number(n).toLocaleString();
  }

  // --- Time range selector ---
  var _selectedRange = '5h'; // '5h' | 'all'
  var timeRangeBtns = document.querySelectorAll('.time-range-btn[data-range]');
  timeRangeBtns.forEach(function (btn) {
    btn.onclick = function () {
      var range = btn.getAttribute('data-range');
      if (range === _selectedRange) return;
      _selectedRange = range;
      timeRangeBtns.forEach(function (b) {
        b.classList.toggle('active', b.getAttribute('data-range') === range);
      });
      loadDashboardStats();
    };
  });

  function getSinceParam() {
    if (_selectedRange === '5h') {
      return Math.floor(Date.now() / 1000) - 5 * 3600;
    }
    return null;
  }

  function loadDashboardStats() {
    var since = getSinceParam();
    var url = '/api/stats';
    if (since != null) url += '?since=' + since;
    fetch(url).then(function (r) { return r.json(); }).then(function (s) {
      clientTotalEl.textContent = formatNumber(s.total_requests);
      if (statTotalTokensEl) statTotalTokensEl.textContent = formatNumber(s.total_tokens);
      if (statPromptTokensEl) statPromptTokensEl.textContent = formatNumber(s.total_prompt_tokens);
      if (statCompletionTokensEl) statCompletionTokensEl.textContent = formatNumber(s.total_completion_tokens);
    }).catch(function () {
      clientTotalEl.textContent = '-';
    });
  }

  function renderClientRequests(requests) {
    loadDashboardStats();
    if (window.Inspection) {
      window.Inspection.renderList(requests || []);
    }
  }

  // --- Timeseries chart ---
  var _tsChart = null;
  var _tsChartCanvas = document.getElementById('timeseries-chart');

  function initChart() {
    if (!_tsChartCanvas || typeof Chart === 'undefined') return;
    var ctx = _tsChartCanvas.getContext('2d');
    var gridColor = 'rgba(48, 54, 61, 0.6)';
    var tickColor = '#8b949e';
    _tsChart = new Chart(ctx, {
      type: 'line',
      data: {
        labels: [],
        datasets: [
          {
            label: t('chartRequests'),
            data: [],
            borderColor: '#58a6ff',
            backgroundColor: 'rgba(88, 166, 255, 0.08)',
            borderWidth: 1.5,
            pointRadius: 0,
            pointHitRadius: 6,
            fill: true,
            tension: 0.3,
            yAxisID: 'y'
          },
          {
            label: t('chartTokens'),
            data: [],
            borderColor: '#3fb950',
            backgroundColor: 'rgba(63, 185, 80, 0.06)',
            borderWidth: 1.5,
            pointRadius: 0,
            pointHitRadius: 6,
            fill: true,
            tension: 0.3,
            yAxisID: 'y1'
          }
        ]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: { mode: 'index', intersect: false },
        plugins: {
          legend: {
            labels: {
              color: tickColor,
              font: { family: "'DM Sans', sans-serif", size: 12 },
              boxWidth: 12,
              padding: 16,
              generateLabels: function (chart) {
                return chart.data.datasets.map(function (ds, i) {
                  var meta = chart.getDatasetMeta(i);
                  var isHidden = meta.hidden === true;
                  return {
                    text: ds.label,
                    fillStyle: isHidden ? 'rgba(128,128,128,0.3)' : ds.backgroundColor,
                    strokeStyle: isHidden ? 'rgba(128,128,128,0.5)' : ds.borderColor,
                    lineWidth: ds.borderWidth || 1.5,
                    hidden: false,
                    fontColor: isHidden ? '#6e7681' : tickColor,
                    datasetIndex: i
                  };
                });
              }
            }
          },
          tooltip: {
            backgroundColor: 'rgba(22, 27, 34, 0.95)',
            titleColor: '#e6edf3',
            bodyColor: '#e6edf3',
            borderColor: '#30363d',
            borderWidth: 1,
            padding: 10,
            callbacks: {
              label: function (ctx) {
                return ctx.dataset.label + ': ' + formatNumber(ctx.parsed.y);
              }
            }
          }
        },
        scales: {
          x: {
            ticks: { color: tickColor, font: { size: 10 }, maxTicksLimit: 12, maxRotation: 0 },
            grid: { color: gridColor }
          },
          y: {
            type: 'linear',
            position: 'left',
            min: 0,
            title: { display: true, text: t('chartRequests'), color: tickColor, font: { size: 11 } },
            ticks: { color: tickColor, font: { size: 10 } },
            grid: { color: gridColor }
          },
          y1: {
            type: 'linear',
            position: 'right',
            min: 0,
            title: { display: true, text: t('chartTokens'), color: tickColor, font: { size: 11 } },
            ticks: { color: tickColor, font: { size: 10 } },
            grid: { drawOnChartArea: false }
          }
        }
      }
    });
  }

  function loadTimeseries() {
    fetch('/api/stats/timeseries?hours=24&bucket=60')
      .then(function (r) { return r.json(); })
      .then(function (data) {
        if (!_tsChart || !data || !data.length) return;
        var labels = data.map(function (b) {
          var d = new Date(b.ts * 1000);
          return String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
        });
        var requests = data.map(function (b) { return b.requests; });
        var tokens = data.map(function (b) { return b.total_tokens; });
        _tsChart.data.labels = labels;
        _tsChart.data.datasets[0].data = requests;
        _tsChart.data.datasets[1].data = tokens;
        _tsChart.update('none');
      })
      .catch(function () {});
  }

  function renderBackendStatus(statusList) {
    if (!statusList || statusList.length === 0) {
      backendStatusEl.innerHTML = '<tr><td colspan="5">' + escapeHtml(t('noBackendsRow')) + '</td></tr>';
      return;
    }
    var rows = statusList.map(function (s) {
      var reachBadge = s.reachable ? '<span class="badge live">' + escapeHtml(t('reachableBadge')) + '</span>' : '<span class="badge down">' + escapeHtml(t('unreachableBadge')) + '</span>';
      var lastAt = s.last_request_at ? new Date(s.last_request_at * 1000).toLocaleString() : '-';
      var err = s.last_error ? (' title="' + escapeHtml(s.last_error) + '"') : '';
      var tokenStr = formatNumber(s.total_tokens);
      return '<tr><td>' + escapeHtml(s.name) + '</td><td' + err + '>' + reachBadge + '</td><td>' + formatNumber(s.request_count) + '</td><td>' + tokenStr + '</td><td>' + lastAt + '</td></tr>';
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

  function isUserEditingRouting() {
    if (!routingRulesListEl) return false;
    if (routingRulesListEl.querySelector('.routing-rule-row.expanded')) return true;
    return false;
  }

  function refreshRoutingTargetSelects() {
    if (!routingRulesListEl) return;
    routingRulesListEl.querySelectorAll('.routing-target-select').forEach(function (sel) {
      var curVal = sel.value;
      var opts = '';
      opts += '<option value="first_available">' + escapeHtml(t('routingTargetFirst')) + '</option>';
      opts += '<option value="round_robin">' + escapeHtml(t('routingTargetRoundRobin')) + '</option>';
      _cachedBackends.forEach(function (b) {
        opts += '<option value="specific:' + escapeHtml(b.id) + '">' + escapeHtml(b.name) + '</option>';
      });
      sel.innerHTML = opts;
      var hasVal = Array.prototype.some.call(sel.options, function (o) { return o.value === curVal; });
      if (hasVal) sel.value = curVal;
    });

    routingRulesListEl.querySelectorAll('.routing-target-label').forEach(function (span) {
      var row = span.closest('.routing-rule-row');
      if (!row) return;
      var idx = row.querySelector('.routing-rule-head') && row.querySelector('.routing-rule-head').getAttribute('data-index');
      if (idx === 'default') {
        span.textContent = targetLabel(_routingConfig.default_target, _routingConfig.default_backend_id);
      } else {
        var i = parseInt(idx, 10);
        var rule = _routingConfig.rules && _routingConfig.rules[i];
        if (rule) span.textContent = targetLabel(rule.target, rule.target_backend_id);
      }
    });
  }

  function loadBackendsAndStatus(force) {
    if (!force && isUserEditingBackends()) return;
    fetch('/api/backends').then(function (r) { return r.json(); }).then(function (list) {
      if (!force && isUserEditingBackends()) return;
      _cachedBackends = list || [];
      if (window.Inspection) window.Inspection.setBackends(list);
      renderBackendsList(list);
      if (isUserEditingRouting()) {
        refreshRoutingTargetSelects();
      } else {
        renderRoutingRules();
      }
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
    var consoleDockEl = document.getElementById('console-dock');
    if (consoleDockEl) {
      var isInspection = tabName === 'inspection';
      consoleDockEl.hidden = !isInspection;
      if (!isInspection) {
        document.body.classList.remove('dock-panel-open');
        var dockPanel = document.getElementById('dock-search-panel');
        if (dockPanel) dockPanel.hidden = true;
      }
    }
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

  // --- Routing configuration ---
  var routingRulesListEl = document.getElementById('routing-rules-list');
  var btnExportRouting = document.getElementById('btn-export-routing');
  var _routingConfig = { rules: [], default_target: 'first_available', default_backend_id: null };
  var _cachedBackends = [];

  function conditionLabel(ct) {
    var m = { model: t('routingCondModel'), model_matches: t('routingCondModelMatches'), length: t('routingCondLength'), regex: t('routingCondRegex') };
    return m[ct] || ct;
  }

  function conditionPlaceholder(ct) {
    var m = { model: t('routingPlaceholderModel'), model_matches: t('routingPlaceholderModelMatches'), length: t('routingPlaceholderLength'), regex: t('routingPlaceholderRegex') };
    return m[ct] || '';
  }

  function targetLabel(tgt, bid) {
    if (tgt === 'first_available') return t('routingTargetFirst');
    if (tgt === 'round_robin') return t('routingTargetRoundRobin');
    if (tgt === 'specific') {
      var b = _cachedBackends.find(function (x) { return x.id === bid; });
      return b ? b.name : (bid || t('routingTargetSpecific'));
    }
    return tgt;
  }

  function buildTargetSelect(selectedTarget, selectedBackendId, idSuffix) {
    var opts = '';
    opts += '<option value="first_available"' + (selectedTarget === 'first_available' ? ' selected' : '') + '>' + escapeHtml(t('routingTargetFirst')) + '</option>';
    opts += '<option value="round_robin"' + (selectedTarget === 'round_robin' ? ' selected' : '') + '>' + escapeHtml(t('routingTargetRoundRobin')) + '</option>';
    _cachedBackends.forEach(function (b) {
      var isSelected = selectedTarget === 'specific' && selectedBackendId === b.id;
      opts += '<option value="specific:' + escapeHtml(b.id) + '"' + (isSelected ? ' selected' : '') + '>' + escapeHtml(b.name) + '</option>';
    });
    return '<select class="routing-target-select" id="routing-target-' + idSuffix + '">' + opts + '</select>';
  }

  function parseTargetValue(val) {
    if (val === 'first_available') return { target: 'first_available', target_backend_id: null };
    if (val === 'round_robin') return { target: 'round_robin', target_backend_id: null };
    if (val.startsWith('specific:')) return { target: 'specific', target_backend_id: val.substring(9) };
    return { target: 'first_available', target_backend_id: null };
  }

  function renderRoutingRules() {
    if (!routingRulesListEl) return;
    var rules = _routingConfig.rules || [];
    var html = '';

    rules.forEach(function (rule, i) {
      html += '<div class="routing-rule-row" data-index="' + i + '">';
      html += '<div class="routing-rule-head" data-index="' + i + '">';
      html += '<span class="routing-rule-num">#' + (i + 1) + '</span>';
      html += '<span class="routing-rule-summary">';
      html += '<span class="routing-kw">' + escapeHtml(t('routingIf')) + '</span> ';
      html += '<span class="routing-cond-label">' + escapeHtml(conditionLabel(rule.condition_type)) + '</span> ';
      html += '<span class="routing-cond-value">' + escapeHtml(rule.condition_value) + '</span> ';
      html += '<span class="routing-kw">' + escapeHtml(t('routingRouteTo')) + '</span> ';
      html += '<span class="routing-target-label">' + escapeHtml(targetLabel(rule.target, rule.target_backend_id)) + '</span>';
      html += '</span>';
      html += '<span class="routing-rule-chevron" aria-hidden="true"></span>';
      html += '</div>';
      html += '<div class="routing-rule-edit" data-index="' + i + '">';
      html += '<div class="routing-rule-fields">';
      html += '<div><label>' + escapeHtml(t('routingIf')) + '</label>';
      html += '<select class="routing-cond-type" id="routing-cond-type-' + i + '">';
      html += '<option value="model"' + (rule.condition_type === 'model' ? ' selected' : '') + '>' + escapeHtml(t('routingCondModel')) + '</option>';
      html += '<option value="model_matches"' + (rule.condition_type === 'model_matches' ? ' selected' : '') + '>' + escapeHtml(t('routingCondModelMatches')) + '</option>';
      html += '<option value="length"' + (rule.condition_type === 'length' ? ' selected' : '') + '>' + escapeHtml(t('routingCondLength')) + '</option>';
      html += '<option value="regex"' + (rule.condition_type === 'regex' ? ' selected' : '') + '>' + escapeHtml(t('routingCondRegex')) + '</option>';
      html += '</select></div>';
      html += '<div><label>' + escapeHtml(t('routingConditionValue')) + '</label>';
      html += '<input type="text" class="routing-cond-value-input" id="routing-cond-value-' + i + '" value="' + escapeHtml(rule.condition_value) + '" placeholder="' + escapeHtml(conditionPlaceholder(rule.condition_type)) + '" /></div>';
      html += '<div><label>' + escapeHtml(t('routingRouteTo')) + '</label>';
      html += buildTargetSelect(rule.target, rule.target_backend_id, i);
      html += '</div>';
      html += '</div>';
      html += '<div class="routing-rule-actions">';
      if (i > 0) html += '<button type="button" class="btn btn-small btn-secondary routing-btn-up" data-index="' + i + '">' + escapeHtml(t('routingMoveUp')) + '</button>';
      if (i < rules.length - 1) html += '<button type="button" class="btn btn-small btn-secondary routing-btn-down" data-index="' + i + '">' + escapeHtml(t('routingMoveDown')) + '</button>';
      html += '<button type="button" class="btn btn-small btn-secondary routing-btn-save" data-index="' + i + '">' + escapeHtml(t('save')) + '</button>';
      html += '<button type="button" class="btn btn-small btn-danger routing-btn-delete" data-index="' + i + '">' + escapeHtml(t('routingDeleteRule')) + '</button>';
      html += '</div></div>';
      html += '</div>';
    });

    // Default row (always present)
    html += '<div class="routing-rule-row routing-default-row">';
    html += '<div class="routing-rule-head routing-default-head" data-index="default">';
    html += '<span class="routing-rule-num routing-default-badge">' + escapeHtml(t('routingDefault')) + '</span>';
    html += '<span class="routing-rule-summary">';
    html += '<span class="routing-kw">' + escapeHtml(t('routingRouteTo')) + '</span> ';
    html += '<span class="routing-target-label">' + escapeHtml(targetLabel(_routingConfig.default_target, _routingConfig.default_backend_id)) + '</span>';
    html += '</span>';
    html += '<span class="routing-rule-chevron" aria-hidden="true"></span>';
    html += '</div>';
    html += '<div class="routing-rule-edit" data-index="default">';
    html += '<div class="routing-rule-fields">';
    html += '<div><label>' + escapeHtml(t('routingRouteTo')) + '</label>';
    html += buildTargetSelect(_routingConfig.default_target, _routingConfig.default_backend_id, 'default');
    html += '</div>';
    html += '</div>';
    html += '<div class="routing-rule-actions">';
    html += '<button type="button" class="btn btn-small btn-secondary routing-btn-save" data-index="default">' + escapeHtml(t('save')) + '</button>';
    html += '</div></div>';
    html += '</div>';

    // Add rule button
    html += '<div class="routing-add-row">';
    html += '<button type="button" class="btn btn-secondary routing-btn-add" id="routing-btn-add">' + escapeHtml(t('routingAddRule')) + '</button>';
    html += '</div>';

    routingRulesListEl.innerHTML = html;
    bindRoutingEvents();
  }

  function bindRoutingEvents() {
    // Update placeholder when condition type changes
    routingRulesListEl.querySelectorAll('.routing-cond-type').forEach(function (sel) {
      sel.onchange = function () {
        var idx = sel.id.replace('routing-cond-type-', '');
        var valInput = document.getElementById('routing-cond-value-' + idx);
        if (valInput) valInput.placeholder = conditionPlaceholder(sel.value);
      };
    });

    // Expand/collapse
    routingRulesListEl.querySelectorAll('.routing-rule-head').forEach(function (head) {
      head.onclick = function () {
        var row = head.parentElement;
        var wasExpanded = row.classList.contains('expanded');
        routingRulesListEl.querySelectorAll('.routing-rule-row').forEach(function (r) { r.classList.remove('expanded'); });
        if (!wasExpanded) row.classList.add('expanded');
      };
    });

    // Move up
    routingRulesListEl.querySelectorAll('.routing-btn-up').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        var i = parseInt(btn.getAttribute('data-index'), 10);
        if (i > 0) {
          collectRuleEdits();
          var tmp = _routingConfig.rules[i];
          _routingConfig.rules[i] = _routingConfig.rules[i - 1];
          _routingConfig.rules[i - 1] = tmp;
          renderRoutingRules();
        }
      };
    });

    // Move down
    routingRulesListEl.querySelectorAll('.routing-btn-down').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        var i = parseInt(btn.getAttribute('data-index'), 10);
        if (i < _routingConfig.rules.length - 1) {
          collectRuleEdits();
          var tmp = _routingConfig.rules[i];
          _routingConfig.rules[i] = _routingConfig.rules[i + 1];
          _routingConfig.rules[i + 1] = tmp;
          renderRoutingRules();
        }
      };
    });

    // Delete
    routingRulesListEl.querySelectorAll('.routing-btn-delete').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        var i = parseInt(btn.getAttribute('data-index'), 10);
        collectRuleEdits();
        confirmModal(t('confirmDeleteRoutingRule')).then(function (ok) {
          if (!ok) return;
          var removed = _routingConfig.rules[i];
          _routingConfig.rules.splice(i, 1);
          // Optimistic UI; rollback if persist fails so client matches server on reload.
          renderRoutingRules();
          saveRoutingConfig().then(function (res) {
            if (!res || !res.ok) {
              _routingConfig.rules.splice(i, 0, removed);
              renderRoutingRules();
            }
          });
        });
      };
    });

    // Save (per-rule)
    routingRulesListEl.querySelectorAll('.routing-btn-save').forEach(function (btn) {
      btn.onclick = function (e) {
        e.stopPropagation();
        saveRoutingConfig().then(function (res) {
          if (res && res.ok) renderRoutingRules();
        });
      };
    });

    // Add rule
    var btnAdd = document.getElementById('routing-btn-add');
    if (btnAdd) {
      btnAdd.onclick = function () {
        collectRuleEdits();
        _routingConfig.rules.push({ condition_type: 'model', condition_value: '', target: 'first_available', target_backend_id: null });
        renderRoutingRules();
        var lastIdx = _routingConfig.rules.length - 1;
        var lastRow = routingRulesListEl.querySelector('.routing-rule-row[data-index="' + lastIdx + '"]');
        if (lastRow) lastRow.classList.add('expanded');
      };
    }
  }

  function collectRuleEdits() {
    (_routingConfig.rules || []).forEach(function (rule, i) {
      var condType = document.getElementById('routing-cond-type-' + i);
      var condVal = document.getElementById('routing-cond-value-' + i);
      var targetSel = document.getElementById('routing-target-' + i);
      if (condType) rule.condition_type = condType.value;
      if (condVal) rule.condition_value = condVal.value;
      if (targetSel) {
        var parsed = parseTargetValue(targetSel.value);
        rule.target = parsed.target;
        rule.target_backend_id = parsed.target_backend_id;
      }
    });
    var defaultTarget = document.getElementById('routing-target-default');
    if (defaultTarget) {
      var parsed = parseTargetValue(defaultTarget.value);
      _routingConfig.default_target = parsed.target;
      _routingConfig.default_backend_id = parsed.target_backend_id;
    }
  }

  function loadRoutingConfig() {
    fetch('/api/routing').then(function (r) { return r.json(); }).then(function (data) {
      _routingConfig = data || { rules: [], default_target: 'first_available', default_backend_id: null };
      renderRoutingRules();
    }).catch(function () {});
  }

  function saveRoutingConfig() {
    collectRuleEdits();
    var body = {
      rules: (_routingConfig.rules || []).map(function (r) {
        var obj = { condition_type: r.condition_type, condition_value: r.condition_value, target: r.target };
        if (r.target_backend_id) obj.target_backend_id = r.target_backend_id;
        return obj;
      }),
      default_target: _routingConfig.default_target
    };
    if (_routingConfig.default_backend_id) body.default_backend_id = _routingConfig.default_backend_id;
    return fetch('/api/routing', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    })
      .then(function (r) {
        return r.json().then(function (res) {
          return { httpOk: r.ok, res: res || {} };
        });
      })
      .then(function (pair) {
        var res = pair.res;
        if (pair.httpOk && res.ok) {
          showToast(t('backendSaved'));
          return { ok: true };
        }
        showToast(t('routingSaveFailed'));
        return { ok: false };
      })
      .catch(function () {
        showToast(t('routingSaveFailed'));
        return { ok: false };
      });
  }

  if (btnExportRouting) {
    btnExportRouting.onclick = function () {
      fetch('/api/routing/export').then(function (r) {
        if (!r.ok) return r.json().then(function (j) { throw new Error(j.error || r.statusText); });
        return r.blob();
      }).then(function (blob) {
        var a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = 'routing.conf';
        a.click();
        URL.revokeObjectURL(a.href);
      }).catch(function (err) {
        showToast(t('routingSaveFailed'));
      });
    };
  }

  function fetchCachedBackendsThen(callback) {
    fetch('/api/backends').then(function (r) { return r.json(); }).then(function (list) {
      _cachedBackends = list || [];
      if (window.Inspection) window.Inspection.setBackends(list);
    }).catch(function () {}).finally(function () {
      if (callback) callback();
    });
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
    fetchCachedBackendsThen(function () {
      loadRoutingConfig();
    });
    renderVersionInfo();
    if (_tsChart) {
      _tsChart.data.datasets[0].label = t('chartRequests');
      _tsChart.data.datasets[1].label = t('chartTokens');
      _tsChart.options.scales.y.title.text = t('chartRequests');
      _tsChart.options.scales.y1.title.text = t('chartTokens');
      _tsChart.update('none');
    }
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
          loadTimeseries();
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
    fetchCachedBackendsThen(function () {
      loadRoutingConfig();
    });
    renderVersionInfo();
    initChart();
    loadTimeseries();
    startNotifyPoll();
    setInterval(function () {
      loadRequests();
      loadBackendsAndStatus();
      loadTimeseries();
    }, FALLBACK_POLL_INTERVAL_MS);
  }

  init();
})();
