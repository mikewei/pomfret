/**
 * Simple i18n: English (default) and Simplified Chinese.
 * Language follows system: zh* -> zh-CN, else en.
 */
(function () {
  const messages = {
    en: {
      appTitle: 'Pomfret Console',
      logo: 'Pomfret',
      tagline: 'Proxy Of Models For Routing, Evaluation & Telemetry',
      connectTitle: 'How to connect (client)',
      connectDesc: 'Point your client (e.g. OpenClaw, Agent UI, SDK) to the base URL below.',
      gatewayUrl: 'Base URL',
      copy: 'Copy',
      jumpToLast: 'Jump to last',
      proxyTitle: 'Proxy settings',
      proxyDesc: 'Affects Pomfret outbound requests to backends.',
      httpProxyLabel: 'HTTP_PROXY',
      httpsProxyLabel: 'HTTPS_PROXY',
      allProxyLabel: 'ALL_PROXY',
      noProxyLabel: 'NO_PROXY',
      openclawConfig: 'OpenClaw Config',
      openclawConfigExampleDesc: 'OpenClaw config example (in models.providers of openclaw.conf)',
      close: 'Close',
      exampleChat: 'Example: Chat completion',
      exampleModels: 'Example: List models',
      backendsTitle: 'Backend LLM configuration',
      backendsDesc: 'Configure and select the backend to forward to; edit Base URL and API Key.',
      noBackends: 'No backends configured. Click "Add backend" below to add one.',
      addBackend: 'Add backend',
      current: 'Current',
      name: 'Name',
      backendType: 'Backend type',
      backendTypeOllama: 'Ollama',
      backendTypeOpenAiCompat: 'OpenAI-compatible',
      backendTypeGemini: 'Google Gemini',
      baseUrl: 'Base URL',
      apiKeyLabel: 'API Key (optional, leave blank to keep)',
      apiKeySet: 'Set',
      apiKeyNotSet: 'Not set',
      specifiedModel: 'Specified model',
      specifiedModelPlaceholder: 'Uses the model from request if not set',
      use: 'Use',
      save: 'Save',
      delete: 'Delete',
      confirmTitle: 'Confirm',
      confirmDeleteBackend: 'Delete this backend?',
      confirmDeleteRoutingRule: 'Delete this routing rule?',
      clientRequests: 'Client requests',
      totalRequests: 'Total requests',
      totalTokens: 'Total tokens',
      promptTokens: 'Prompt tokens',
      completionTokens: 'Completion tokens',
      tokens: 'Tokens',
      rangeLast5h: 'Last 5h',
      rangeSinceStartup: 'Since startup',
      chartTitle: 'Requests & Tokens',
      chartRequests: 'Requests',
      chartTokens: 'Tokens',
      time: 'Time',
      method: 'Method',
      path: 'Path',
      backend: 'Backend',
      model: 'Model',
      backendModel: 'Backend (Model)',
      status: 'Status',
      backendConnectivity: 'Backend connectivity & usage',
      refreshStatus: 'Refresh status',
      reachable: 'Reachable',
      requests: 'Requests',
      lastRequest: 'Last request',
      noBackendsRow: 'No backends',
      reachableBadge: 'Reachable',
      unreachableBadge: 'Unreachable',
      noRequests: 'No requests yet',
      detail: 'Detail',
      loadFailed: 'Load failed',
      detailTitle: 'Request detail',
      back: 'Back',
      refreshAll: 'Refresh all',
      loading: 'Loading…',
      notFound: 'Request not found',
      requestBody: 'Request body',
      responseBody: 'Response body',
      none: '(none)',
      saveConfig: 'Save config',
      export: 'Export',
      cancel: 'Cancel',
      configSaved: 'Config saved.',
      configSaveFailed: 'Failed to save config',
      backendSaved: 'Saved',
      tabConfiguration: 'Configuration',
      tabDashboard: 'Dashboard',
      dashboardTitle: 'Overview',
      tabInspection: 'Inspection',
      dashboardDesc: 'Connection status and backend usage overview.',
      inspectionDesc: 'Request history and detailed analysis.',
      inspTraceList: 'Trace list',
      inspOverview: 'Overview',
      inspRequest: 'Request (params & headers)',
      inspBackend: 'Backend',
      inspResponse: 'Response',
      inspRequestBodySize: 'Request body size',
      inspResponseBodySize: 'Response body size',
      dockSearchTitle: 'Search requests',
      dockSearchPlaceholder: 'Keyword in body',
      dockSearch: 'Search',
      dockClear: 'Clear',
      dockRecords: 'Records',
      dockMatches: 'Matches',
      dockPrevRecord: 'Previous',
      dockNextRecord: 'Next',
      dockPrevMatch: 'Previous',
      dockNextMatch: 'Next',
      dockSearching: 'Searching…',
      dockNoResults: 'No matching records.',
      dockSearchFailed: 'Search failed.',
      dockQueryTooLong: 'Query too long (max 256 characters).',
      dockTruncatedHint: 'list truncated',
      dockStatsTotal: 'Matched records',
      dockStatsRecordNav: 'Record',
      dockStatsMatchNav: 'Match',
      dockNoMatchesInRecord: 'No matches in this record body.',
      dockPanelClose: 'Close panel',
      connectionLost: 'Connection lost. Reconnecting…',
      connectionRetry: 'Retry',
      versionFrontend: 'Frontend',
      versionBackend: 'Backend',
      versionUnknown: '?',
      routingTitle: 'Routing configuration',
      routingDesc: 'Route requests to backends based on conditions. Rules are evaluated top-to-bottom; the first match wins.',
      routingAddRule: 'Add rule',
      routingDefault: 'Default',
      routingIf: 'If',
      routingRouteTo: 'route to',
      routingCondModel: 'Model ==',
      routingCondLength: 'Prompt length >',
      routingCondRegex: 'Prompt matches',
      routingPlaceholderModel: 'Model name',
      routingPlaceholderLength: 'Bytes',
      routingPlaceholderRegex: 'Regex',
      routingTargetFirst: 'First available',
      routingTargetRoundRobin: 'Round robin',
      routingTargetSpecific: 'Specific backend',
      routingSaved: 'Routing saved',
      routingSaveFailed: 'Failed to save routing',
      routingMoveUp: 'Up',
      routingMoveDown: 'Down',
      routingDeleteRule: 'Delete',
      routingConditionValue: 'Value',
      routingSave: 'Save routing'
    },
    'zh-CN': {
      appTitle: 'Pomfret 控制台',
      logo: 'Pomfret',
      tagline: 'Proxy Of Models For Routing, Evaluation & Telemetry',
      connectTitle: '如何连接（客户端）',
      connectDesc: '将客户端（如 OpenClaw、Agent UI、SDK）指向下方 Base URL。',
      gatewayUrl: 'Base URL',
      copy: '复制',
      jumpToLast: '跳到最后',
      proxyTitle: '代理设置',
      proxyDesc: '影响 Pomfret 访问后端的出站请求。',
      httpProxyLabel: 'HTTP_PROXY',
      httpsProxyLabel: 'HTTPS_PROXY',
      allProxyLabel: 'ALL_PROXY',
      noProxyLabel: 'NO_PROXY',
      openclawConfig: 'OpenClaw 配置',
      openclawConfigExampleDesc: 'OpenClaw 配置示例（填入 openclaw.conf 的 models.providers）',
      close: '关闭',
      exampleChat: '示例：对话补全',
      exampleModels: '示例：列出模型',
      backendsTitle: '后端 LLM 配置',
      backendsDesc: '配置并选择要转发的后端；可编辑 Base URL 与 API Key。',
      noBackends: '暂无已配置的后端，请点击下方「添加后端」进行添加。',
      addBackend: '添加后端',
      current: '当前',
      name: '名称',
      backendType: '后端类型',
      backendTypeOllama: 'Ollama',
      backendTypeOpenAiCompat: 'OpenAI 兼容',
      backendTypeGemini: 'Google Gemini',
      baseUrl: 'Base URL',
      apiKeyLabel: 'API Key（可选，留空则保持原样）',
      apiKeySet: '已设置',
      apiKeyNotSet: '未设置',
      specifiedModel: '指定模型',
      specifiedModelPlaceholder: '如果不设置则使用请求参数指定的模型',
      use: '使用',
      save: '保存',
      delete: '删除',
      confirmTitle: '确认',
      confirmDeleteBackend: '确定要删除该后端吗？',
      confirmDeleteRoutingRule: '确定要删除该路由规则吗？',
      clientRequests: '客户端请求',
      totalRequests: '总请求数',
      totalTokens: '总 Token 数',
      promptTokens: 'Prompt Token',
      completionTokens: 'Completion Token',
      tokens: 'Token 数',
      rangeLast5h: '最近 5 小时',
      rangeSinceStartup: '启动以来',
      chartTitle: '请求与 Token 趋势',
      chartRequests: '请求数',
      chartTokens: 'Token 数',
      time: '时间',
      method: '方法',
      path: '路径',
      backend: '后端',
      model: '模型',
      backendModel: '后端 (模型)',
      status: '状态',
      backendConnectivity: '后端连通性与使用',
      refreshStatus: '刷新状态',
      reachable: '可达',
      requests: '请求数',
      lastRequest: '最后请求',
      noBackendsRow: '无后端',
      reachableBadge: '可达',
      unreachableBadge: '不可达',
      noRequests: '暂无请求',
      detail: '详情',
      loadFailed: '加载失败',
      detailTitle: '请求详情',
      back: '返回',
      refreshAll: '全部刷新',
      loading: '加载中…',
      notFound: '未找到请求',
      requestBody: '请求体',
      responseBody: '响应体',
      none: '（无）',
      saveConfig: '保存配置',
      export: '导出',
      cancel: '取消',
      configSaved: '配置已保存。',
      configSaveFailed: '保存配置失败',
      backendSaved: '已保存',
      tabConfiguration: '配置',
      tabDashboard: '仪表盘',
      dashboardTitle: '概览',
      tabInspection: '请求分析',
      dashboardDesc: '连接状态与后端使用概览。',
      inspectionDesc: '请求历史与详细分析。',
      inspTraceList: '追踪列表',
      inspOverview: '概览',
      inspRequest: '请求（参数与头）',
      inspBackend: '后端',
      inspResponse: '响应',
      inspRequestBodySize: '请求体大小',
      inspResponseBodySize: '响应体大小',
      dockSearchTitle: '搜索请求',
      dockSearchPlaceholder: '请求/响应体关键字',
      dockSearch: '搜索',
      dockClear: '清除',
      dockRecords: '记录',
      dockMatches: '命中',
      dockPrevRecord: '上一条',
      dockNextRecord: '下一条',
      dockPrevMatch: '上一处',
      dockNextMatch: '下一处',
      dockSearching: '搜索中…',
      dockNoResults: '没有匹配的记录。',
      dockSearchFailed: '搜索失败。',
      dockQueryTooLong: '关键字过长（最多 256 个字符）。',
      dockTruncatedHint: '列表已截断',
      dockStatsTotal: '命中记录数',
      dockStatsRecordNav: '记录',
      dockStatsMatchNav: '命中',
      dockNoMatchesInRecord: '当前记录正文中无匹配。',
      dockPanelClose: '关闭面板',
      connectionLost: '无法连接到服务器，正在重试…',
      connectionRetry: '重试',
      versionFrontend: '前端',
      versionBackend: '后端',
      versionUnknown: '?',
      routingTitle: '路由配置',
      routingDesc: '根据条件将请求路由到不同后端。规则从上到下依次匹配，第一个匹配的规则生效。',
      routingAddRule: '添加规则',
      routingDefault: '默认',
      routingIf: '如果',
      routingRouteTo: '路由到',
      routingCondModel: 'Model ==',
      routingCondLength: 'Prompt length >',
      routingCondRegex: 'Prompt matches',
      routingPlaceholderModel: '模型名称',
      routingPlaceholderLength: '字节数',
      routingPlaceholderRegex: '正则表达式',
      routingTargetFirst: '首个可用',
      routingTargetRoundRobin: '轮询',
      routingTargetSpecific: '指定后端',
      routingSaved: '路由已保存',
      routingSaveFailed: '路由保存失败',
      routingMoveUp: '上移',
      routingMoveDown: '下移',
      routingDeleteRule: '删除',
      routingConditionValue: '值',
      routingSave: '保存路由'
    }
  };

  var STORAGE_KEY = 'pomfret-lang';

  function detectLang() {
    var nav = window.navigator;
    var lang = (nav.language || nav.userLanguage || '').toLowerCase();
    if (lang.startsWith('zh')) return 'zh-CN';
    if (nav.languages && nav.languages.length) {
      for (var i = 0; i < nav.languages.length; i++) {
        if (String(nav.languages[i]).toLowerCase().startsWith('zh')) return 'zh-CN';
      }
    }
    return 'en';
  }

  var currentLang = (function () {
    try {
      var saved = localStorage.getItem(STORAGE_KEY);
      if (saved === 'en' || saved === 'zh-CN') return saved;
    } catch (e) {}
    return detectLang();
  })();
  var dict = messages[currentLang] || messages.en;

  function t(key) {
    return dict[key] != null ? dict[key] : (messages.en[key] != null ? messages.en[key] : key);
  }

  function applyTranslations() {
    document.documentElement.lang = currentLang === 'zh-CN' ? 'zh-CN' : 'en';
    var titleEl = document.querySelector('title');
    if (titleEl) titleEl.textContent = t('appTitle');
    document.querySelectorAll('[data-i18n]').forEach(function (el) {
      var key = el.getAttribute('data-i18n');
      var val = t(key);
      if (val !== key) {
        if (el.getAttribute('data-i18n-placeholder')) el.placeholder = val;
        else el.textContent = val;
      }
    });
    document.querySelectorAll('.lang-btn').forEach(function (btn) {
      var lang = btn.getAttribute('data-lang');
      btn.classList.toggle('active', lang === currentLang);
    });
  }

  function setLang(lang) {
    if (lang !== 'en' && lang !== 'zh-CN') return;
    try { localStorage.setItem(STORAGE_KEY, lang); } catch (e) {}
    currentLang = lang;
    dict = messages[lang] || messages.en;
    if (window.i18n) window.i18n.lang = currentLang;
    applyTranslations();
    try { document.dispatchEvent(new CustomEvent('i18n:changed')); } catch (e) {}
  }

  window.i18n = { t: t, lang: currentLang, setLang: setLang };
  document.querySelectorAll('.lang-btn').forEach(function (btn) {
    btn.onclick = function () { setLang(btn.getAttribute('data-lang')); };
  });
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', applyTranslations);
  } else {
    applyTranslations();
  }
})();
