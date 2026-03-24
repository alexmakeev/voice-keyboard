// Voice Keyboard UI Application

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// State
let transcriptions = [];
let debugLines = [];
const MAX_DEBUG_LINES = 2000;
let statusPollTimer = null;
let lastPollStatus = null;
let lastPollTranscriptionCount = 0;
let lastPollDebugCount = 0;
let doneTimeout = null;
const _updateState = {
    downloadUrl: null,
    checksumsUrl: null,
    assetFilename: null,
    pendingInfo: null,
};
let debugFilters = {
    all: true,
    system: true,
    recording: true,
    vad: true,
    worker: true,
    filter: true,
    transcription: true,
    error: true,
    phrase: true,
};
let config = {
    model: 'large-v3-turbo',
    language: 'ru',
    hotkey: 'fn',
    input_method: 'keyboard',
    openai_api_key: '',
    openai_api_url: 'https://api.openai.com/v1',
    transcription_mode: 'openai',
    sound_enabled: true,
    audio_device: '',
    lower_volume_on_record: true,
    min_recording_ms: 1000,
    preprompt_default: '',
    preprompt_1: '',
    preprompt_2: '',
    preprompt_3: '',
};

// Models configuration
const MODELS = [
    { id: 'tiny', name: 'Tiny', desc: 'Fastest, lowest accuracy', size: '75 MB' },
    { id: 'base', name: 'Base', desc: 'Fast, good accuracy', size: '142 MB' },
    { id: 'small', name: 'Small', desc: 'Balanced speed/accuracy', size: '466 MB' },
    { id: 'medium', name: 'Medium', desc: 'High accuracy, slower', size: '1.5 GB' },
    { id: 'large-v3-turbo', name: 'Large v3 Turbo', desc: 'Best accuracy', size: '1.6 GB' }
];

const LANGUAGES = [
    { code: 'auto', name: 'Auto-detect' },
    { code: 'en', name: 'English' },
    { code: 'ru', name: 'Russian' },
    { code: 'de', name: 'German' },
    { code: 'fr', name: 'French' },
    { code: 'es', name: 'Spanish' },
    { code: 'it', name: 'Italian' },
    { code: 'pt', name: 'Portuguese' },
    { code: 'zh', name: 'Chinese' },
    { code: 'ja', name: 'Japanese' },
    { code: 'ko', name: 'Korean' }
];

// DOM Elements
let elements = {};

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
    cacheElements();
    setupTabs();
    setupEventListeners();
    setupDebugFilters();
    setupModeSelector();
    setupPermissionsListeners();
    setupTauriListeners().catch(e => console.error('Event listeners failed:', e));
    loadVersionInfo().catch(e => console.error('Failed to load version info:', e));
    await loadConfig();
    await checkPermissions();
    await loadTranscriptions();
    await loadDebugLog();
    renderModels();
    renderLanguages();
    startStatusPolling();
});

function cacheElements() {
    elements = {
        tabs: document.querySelectorAll('.tab'),
        tabContents: document.querySelectorAll('.tab-content'),
        // Test tab
        testConnection: document.getElementById('test-connection'),
        testMode: document.getElementById('test-mode'),
        testHotkey: document.getElementById('test-hotkey'),
        hotkeyState: document.getElementById('hotkey-state'),
        hotkeyIcon: document.querySelector('.hotkey-icon'),
        hotkeyLabel: document.querySelector('.hotkey-label'),
        testResultText: document.getElementById('test-result-text'),
        // Log tab (debug)
        debugLog: document.getElementById('debug-log'),
        debugLogContainer: document.querySelector('.debug-log-container'),
        debugAutoscroll: document.getElementById('debug-autoscroll'),
        clearDebugBtn: document.getElementById('clear-debug'),
        // Settings
        modelsList: document.getElementById('models-list'),
        modelSettings: document.getElementById('model-settings'),
        openaiSettings: document.getElementById('openai-settings'),
        languageSelect: document.getElementById('language-select'),
        hotkeySelect: document.getElementById('hotkey-select'),
        inputMethodSelect: document.getElementById('input-method-select'),
        openaiKeyInput: document.getElementById('openai-key'),
        openaiUrlInput: document.getElementById('openai-url'),
        soundEnabled: document.getElementById('sound-enabled'),
        audioDeviceSelect: document.getElementById('audio-device-select'),
        lowerVolume: document.getElementById('lower-volume'),
        minRecordingMs: document.getElementById('min-recording-ms'),
        prepromptDefault: document.getElementById('preprompt-default'),
        preprompt1: document.getElementById('preprompt-1'),
        preprompt2: document.getElementById('preprompt-2'),
        preprompt3: document.getElementById('preprompt-3'),
        saveSettingsBtn: document.getElementById('save-settings'),
        savePromptsBtn: document.getElementById('save-prompts'),
        // App info
        appVersion: document.getElementById('app-version'),
        appUpdateStatus: document.getElementById('app-update-status'),
        checkUpdateBtn: document.getElementById('check-update-btn'),
        // Permissions modal
        permissionsModal: document.getElementById('permissions-modal'),
        openSettingsBtn: document.getElementById('open-settings-btn'),
        checkAgainBtn: document.getElementById('check-again-btn'),
        // Report modal
        reportModal: document.getElementById('report-modal'),
        cancelReportBtn: document.getElementById('cancel-report'),
        createReportBtn: document.getElementById('create-report'),
        modeCards: document.querySelectorAll('.mode-card'),
    };
}

function setupTabs() {
    elements.tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const tabId = tab.dataset.tab;

            // Update tab buttons
            elements.tabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');

            // Update tab content
            elements.tabContents.forEach(content => {
                content.classList.remove('active');
                if (content.id === `${tabId}-tab`) {
                    content.classList.add('active');
                }
            });

            // When switching to log tab, render and scroll
            if (tabId === 'log') {
                renderDebugLog();
            }
        });
    });
}

function setupEventListeners() {
    // Clear debug log
    elements.clearDebugBtn.addEventListener('click', async () => {
        debugLines = [];
        renderDebugLog();
        try {
            await invoke('clear_debug_log');
        } catch (e) {
            console.error('Failed to clear debug log:', e);
        }
    });

    // Report modal (triggered from tray menu)
    elements.cancelReportBtn.addEventListener('click', () => {
        elements.reportModal.classList.add('hidden');
    });

    elements.createReportBtn.addEventListener('click', async () => {
        elements.createReportBtn.disabled = true;
        elements.createReportBtn.textContent = 'Creating...';

        try {
            const zipPath = await invoke('create_debug_report');
            await invoke('open_github_issue', { zipPath });
            elements.reportModal.classList.add('hidden');
        } catch (e) {
            console.error('Failed to create report:', e);
            alert('Failed to create debug report: ' + e);
        } finally {
            elements.createReportBtn.disabled = false;
            elements.createReportBtn.textContent = 'Create & Open GitHub Issue';
        }
    });

    // Save settings
    elements.saveSettingsBtn.addEventListener('click', saveSettings);
    elements.savePromptsBtn.addEventListener('click', saveSettings);

    // Settings changes
    elements.languageSelect.addEventListener('change', (e) => {
        config.language = e.target.value;
    });

    elements.hotkeySelect.addEventListener('change', (e) => {
        config.hotkey = e.target.value;
        updateHotkeyHint();
    });

    elements.inputMethodSelect.addEventListener('change', (e) => {
        config.input_method = e.target.value;
    });

    elements.openaiKeyInput.addEventListener('input', (e) => {
        config.openai_api_key = e.target.value;
        updateApiKeyHint();
    });

    elements.openaiUrlInput.addEventListener('input', (e) => {
        config.openai_api_url = e.target.value;
    });

    elements.soundEnabled.addEventListener('change', (e) => {
        config.sound_enabled = e.target.checked;
    });

    elements.audioDeviceSelect.addEventListener('change', (e) => {
        config.audio_device = e.target.value;
    });

    elements.lowerVolume.addEventListener('change', (e) => {
        config.lower_volume_on_record = e.target.checked;
    });

    elements.minRecordingMs.addEventListener('change', (e) => {
        let val = parseInt(e.target.value, 10);
        if (isNaN(val) || val < 100) val = 100;
        if (val > 5000) val = 5000;
        e.target.value = val;
        config.min_recording_ms = val;
    });
}

function setupDebugFilters() {
    document.querySelectorAll('.filter-chip').forEach(chip => {
        chip.addEventListener('click', () => {
            const filter = chip.dataset.filter;
            const checkbox = chip.querySelector('input');

            if (filter === 'all') {
                const newState = !debugFilters.all;
                debugFilters.all = newState;
                // Toggle all filters
                Object.keys(debugFilters).forEach(k => debugFilters[k] = newState);
                document.querySelectorAll('.filter-chip').forEach(c => {
                    c.classList.toggle('active', newState);
                    c.querySelector('input').checked = newState;
                });
            } else {
                debugFilters[filter] = !debugFilters[filter];
                chip.classList.toggle('active', debugFilters[filter]);
                checkbox.checked = debugFilters[filter];

                // Update "All" state
                const allActive = Object.entries(debugFilters)
                    .filter(([k]) => k !== 'all')
                    .every(([, v]) => v);
                debugFilters.all = allActive;
                const allChip = document.querySelector('.filter-chip[data-filter="all"]');
                allChip.classList.toggle('active', allActive);
                allChip.querySelector('input').checked = allActive;
            }

            renderDebugLog();
        });
    });
}

function setupModeSelector() {
    elements.modeCards.forEach(card => {
        card.addEventListener('click', () => {
            const mode = card.dataset.mode;
            config.transcription_mode = mode;

            // Update card selection
            elements.modeCards.forEach(c => c.classList.remove('selected'));
            card.classList.add('selected');
            card.querySelector('input').checked = true;

            // Toggle settings sections
            updateModeVisibility();
            updateTestMode();
        });
    });
}

function updateModeVisibility() {
    if (config.transcription_mode === 'openai') {
        elements.openaiSettings.classList.remove('hidden');
        elements.modelSettings.classList.add('hidden');
    } else {
        elements.openaiSettings.classList.add('hidden');
        elements.modelSettings.classList.remove('hidden');
    }
}

async function setupTauriListeners() {
    // Listen for status updates
    await listen('status-update', (event) => {
        const payload = event.payload;
        lastPollStatus = payload.status + ':' + payload.text;
        updateStatus(payload.status, payload.text);
        updateConnectionBadge(payload.status);
    });

    // Listen for new transcriptions
    await listen('transcription', (event) => {
        addTranscription(event.payload);
    });

    // Listen for debug log lines
    await listen('debug-log', (event) => {
        addDebugLine(event.payload);
    });

    // Listen for navigation requests (from tray menu)
    await listen('navigate', (event) => {
        const tabId = event.payload;
        const tab = document.querySelector(`.tab[data-tab="${tabId}"]`);
        if (tab) tab.click();
    });

    // Listen for report creation request (from tray menu)
    await listen('create-report', () => {
        elements.reportModal.classList.remove('hidden');
    });

    // Listen for model download progress
    await listen('model-download-progress', (event) => {
        const { model_id, downloaded, total } = event.payload;
        const bar = document.getElementById(`progress-${model_id}`);
        const text = document.getElementById(`progress-text-${model_id}`);
        if (bar) {
            if (total > 0) {
                const pct = Math.round((downloaded / total) * 100);
                bar.style.width = pct + '%';
                if (text) text.textContent = pct + '%';
            } else {
                // No Content-Length — show downloaded size and animate bar
                bar.style.width = '100%';
                bar.style.opacity = '0.6';
                const mb = (downloaded / 1048576).toFixed(1);
                if (text) text.textContent = `${mb} MB`;
            }
        } else {
            console.warn(`[download] progress bar element not found for model=${model_id}`);
        }
    });

    // Listen for update-available event from backend
    await listen('update-available', (event) => {
        const payload = event.payload;
        if (payload && payload.version) {
            storeUpdateInfo(payload);
            setUpdateStatusClickable('update-available', `New version available: v${payload.version}`);
        }
    });

    // Listen for update download/install progress
    await listen('update-progress', (event) => {
        const stage = event.payload.stage;
        const progressText = document.getElementById('update-progress-text');
        const progressBar = document.getElementById('update-progress-bar');
        if (stage === 'downloading') {
            if (progressText) progressText.textContent = 'Downloading update...';
            if (progressBar) progressBar.style.width = '33%';
        } else if (stage === 'installing') {
            if (progressText) progressText.textContent = 'Installing update...';
            if (progressBar) progressBar.style.width = '66%';
        } else if (stage === 'restarting') {
            if (progressText) progressText.textContent = 'Restarting application...';
            if (progressBar) progressBar.style.width = '100%';
        }
    });

    // Listen for model download completion
    await listen('model-download-complete', (event) => {
        const { model_id, success, error } = event.payload;
        downloadingModels.delete(model_id);
        if (!success) {
            console.error(`Model download failed: ${error}`);
            const actionEl = document.getElementById(`action-${model_id}`);
            if (actionEl) {
                actionEl.innerHTML = `<span class="model-status not-downloaded">Failed</span>`;
            }
            setTimeout(() => checkModelStatuses(), 2000);
        } else {
            checkModelStatuses();
        }
    });
}

async function loadConfig() {
    try {
        const savedConfig = await invoke('get_config');
        if (savedConfig) {
            if (savedConfig.inputMethod && !savedConfig.input_method) {
                savedConfig.input_method = savedConfig.inputMethod;
            }
            config = { ...config, ...savedConfig };
        }
    } catch (e) {
        console.error('Failed to load config:', e);
    }

    // Apply config to UI
    elements.hotkeySelect.value = config.hotkey;
    elements.inputMethodSelect.value = config.input_method;
    elements.openaiKeyInput.value = config.openai_api_key || '';
    elements.openaiUrlInput.value = config.openai_api_url || '';
    updateApiKeyHint();
    elements.soundEnabled.checked = config.sound_enabled !== false;
    await loadAudioDevices();
    elements.lowerVolume.checked = config.lower_volume_on_record !== false;
    elements.minRecordingMs.value = config.min_recording_ms || 1000;
    elements.prepromptDefault.value = config.preprompt_default || '';
    elements.preprompt1.value = config.preprompt_1 || '';
    elements.preprompt2.value = config.preprompt_2 || '';
    elements.preprompt3.value = config.preprompt_3 || '';
    updateHotkeyHint();
    updateTestMode();

    // Apply transcription mode
    elements.modeCards.forEach(card => {
        const isSelected = card.dataset.mode === config.transcription_mode;
        card.classList.toggle('selected', isSelected);
        card.querySelector('input').checked = isSelected;
    });
    updateModeVisibility();
}

async function loadTranscriptions() {
    try {
        transcriptions = await invoke('get_transcriptions');
    } catch (e) {
        console.error('Failed to load transcriptions:', e);
        transcriptions = [];
    }
    renderTranscriptions();
}

async function loadDebugLog() {
    try {
        debugLines = await invoke('get_debug_log');
    } catch (e) {
        console.error('Failed to load debug log:', e);
        debugLines = [];
    }
}

function renderTranscriptions() {
    const el = elements.testResultText;
    if (!el) return;

    if (transcriptions.length === 0) {
        el.innerHTML = '<p class="placeholder">Transcription will appear here...</p>';
        return;
    }

    const last = transcriptions[transcriptions.length - 1];
    el.innerHTML = `<div class="text">${escapeHtml(last.text)}</div>`;
}

function addTranscription(transcription) {
    transcriptions.push(transcription);
    renderTranscriptions();
}

function addDebugLine(line) {
    debugLines.push(line);
    if (debugLines.length > MAX_DEBUG_LINES) {
        debugLines = debugLines.slice(-MAX_DEBUG_LINES);
    }

    // Only render if log tab is visible
    const debugTab = document.getElementById('log-tab');
    if (debugTab.classList.contains('active')) {
        appendDebugLineToDOM(line);
    }
}

function renderDebugLog() {
    const filtered = debugLines.filter(line => debugFilters[line.category] !== false);
    elements.debugLog.innerHTML = filtered.map(line => formatDebugLine(line)).join('');

    if (elements.debugAutoscroll.checked) {
        elements.debugLogContainer.scrollTop = elements.debugLogContainer.scrollHeight;
    }
}

function appendDebugLineToDOM(line) {
    if (debugFilters[line.category] === false) return;

    const html = formatDebugLine(line);
    elements.debugLog.insertAdjacentHTML('beforeend', html);

    if (elements.debugAutoscroll.checked) {
        elements.debugLogContainer.scrollTop = elements.debugLogContainer.scrollHeight;
    }
}

function formatDebugLine(line) {
    return `<div class="debug-line cat-${escapeHtml(line.category)}"><span class="dl-time">${escapeHtml(line.timestamp)}</span><span class="dl-msg">${escapeHtml(line.message)}</span></div>`;
}

// Track which models are currently downloading
const downloadingModels = new Set();

function renderModels() {
    elements.modelsList.innerHTML = MODELS.map(model => `
        <div class="model-item ${config.model === model.id ? 'selected' : ''}" data-model="${model.id}">
            <div class="radio"></div>
            <div class="model-info">
                <div class="model-name">${model.name}</div>
                <div class="model-desc">${model.desc}</div>
            </div>
            <div class="model-size">${model.size}</div>
            <div class="model-action" id="action-${model.id}">
                <span class="model-status">Checking...</span>
            </div>
        </div>
    `).join('');

    // Add click handlers for model selection (on the row itself, not buttons)
    elements.modelsList.querySelectorAll('.model-item').forEach(item => {
        item.addEventListener('click', (e) => {
            // Don't select model when clicking action buttons
            if (e.target.closest('.btn-download') || e.target.closest('.btn-delete')) return;
            elements.modelsList.querySelectorAll('.model-item').forEach(i => i.classList.remove('selected'));
            item.classList.add('selected');
            config.model = item.dataset.model;
        });
    });

    // Check model statuses
    checkModelStatuses();
}

async function checkModelStatuses() {
    for (const model of MODELS) {
        const actionEl = document.getElementById(`action-${model.id}`);
        if (!actionEl) continue;
        if (downloadingModels.has(model.id)) continue; // Don't overwrite progress bar
        try {
            const filename = `ggml-${model.id}.bin`;
            const isDownloaded = await invoke('check_model_exists', { modelName: filename });
            if (isDownloaded) {
                actionEl.innerHTML = `<button class="btn btn-small btn-delete" data-model="${model.id}">Delete</button>`;
                actionEl.querySelector('.btn-delete').addEventListener('click', (e) => {
                    e.stopPropagation();
                    deleteModel(model.id);
                });
            } else {
                actionEl.innerHTML = `<button class="btn btn-small btn-download" data-model="${model.id}">Download</button>`;
                actionEl.querySelector('.btn-download').addEventListener('click', (e) => {
                    e.stopPropagation();
                    downloadModel(model.id);
                });
            }
        } catch (e) {
            actionEl.innerHTML = '<span class="model-status">Unknown</span>';
        }
    }
}

async function downloadModel(modelId) {
    downloadingModels.add(modelId);
    const actionEl = document.getElementById(`action-${modelId}`);
    if (actionEl) {
        actionEl.innerHTML = `<div class="model-progress"><div class="model-progress-bar" id="progress-${modelId}"></div></div><span class="model-progress-text" id="progress-text-${modelId}">0%</span>`;
    }
    try {
        await invoke('download_model', { modelId });
        // Command now runs the full download, so completion means success
        downloadingModels.delete(modelId);
        checkModelStatuses();
    } catch (e) {
        console.error('Failed to download model:', e);
        downloadingModels.delete(modelId);
        if (actionEl) {
            actionEl.innerHTML = `<span class="model-status not-downloaded">Failed</span>`;
        }
        setTimeout(() => checkModelStatuses(), 2000);
    }
}

async function deleteModel(modelId) {
    try {
        await invoke('delete_model', { modelId });
        checkModelStatuses();
    } catch (e) {
        console.error('Failed to delete model:', e);
        alert('Failed to delete model: ' + e);
    }
}

async function loadAudioDevices() {
    try {
        const devices = await invoke('get_audio_devices');
        const select = elements.audioDeviceSelect;
        select.textContent = '';
        for (const d of devices) {
            const opt = document.createElement('option');
            opt.value = d.id;
            opt.textContent = d.name;
            if (config.audio_device === d.id) opt.selected = true;
            select.appendChild(opt);
        }
    } catch (e) {
        console.error('Failed to load audio devices:', e);
    }
}

function renderLanguages() {
    elements.languageSelect.innerHTML = LANGUAGES.map(lang =>
        `<option value="${lang.code}" ${config.language === lang.code ? 'selected' : ''}>${lang.name}</option>`
    ).join('');
}

async function saveSettings() {
    const saveButtons = [elements.saveSettingsBtn, elements.savePromptsBtn];
    saveButtons.forEach(btn => { btn.disabled = true; btn.textContent = 'Reloading...'; });

    try {
        config.input_method = elements.inputMethodSelect.value;
        config.openai_api_key = elements.openaiKeyInput.value.trim();
        config.openai_api_url = elements.openaiUrlInput.value.trim();
        config.sound_enabled = elements.soundEnabled.checked;
        config.audio_device = elements.audioDeviceSelect.value;
        config.lower_volume_on_record = elements.lowerVolume.checked;
        let minRec = parseInt(elements.minRecordingMs.value, 10);
        if (isNaN(minRec) || minRec < 100) minRec = 100;
        if (minRec > 5000) minRec = 5000;
        config.min_recording_ms = minRec;
        config.preprompt_default = elements.prepromptDefault.value;
        config.preprompt_1 = elements.preprompt1.value;
        config.preprompt_2 = elements.preprompt2.value;
        config.preprompt_3 = elements.preprompt3.value;
        await invoke('save_config', { config });
        saveButtons.forEach(btn => { btn.textContent = 'Saved!'; });
        setTimeout(() => {
            saveButtons.forEach(btn => { btn.textContent = 'Save & Reload'; btn.disabled = false; });
        }, 1500);
    } catch (e) {
        console.error('Failed to save config:', e);
        alert('Failed to save settings: ' + e);
        saveButtons.forEach(btn => { btn.textContent = 'Save & Reload'; btn.disabled = false; });
    }
}

function updateStatus(status, text) {
    const state = elements.hotkeyState;
    const icon = elements.hotkeyIcon;
    const label = elements.hotkeyLabel;
    if (!state) return;

    state.className = 'hotkey-state';

    // Clear done timer if a new active status arrives
    if (status !== 'done' && status !== 'idle' && doneTimeout) {
        clearTimeout(doneTimeout);
        doneTimeout = null;
    }

    switch (status) {
        case 'recording':
            state.classList.add('recording');
            icon.textContent = '🔴';
            label.innerHTML = 'Listening...';
            break;
        case 'sending':
        case 'processing':
            state.classList.add('processing');
            icon.textContent = '⏳';
            label.innerHTML = 'Transcribing...';
            break;
        case 'improving':
            state.classList.add('processing');
            icon.textContent = '✨';
            label.innerHTML = 'Improving...';
            break;
        case 'typing':
            state.classList.add('processing');
            icon.textContent = '⌨️';
            label.innerHTML = 'Typing...';
            break;
        case 'done':
            // Skip "Done" display, go directly to idle
            if (doneTimeout) clearTimeout(doneTimeout);
            doneTimeout = null;
            state.classList.add('idle');
            icon.textContent = '⏺';
            label.innerHTML = `Press and hold <kbd>${getHotkeyName()}</kbd> to record`;
            break;
        case 'connecting':
            state.classList.add('idle');
            icon.textContent = '⏳';
            label.innerHTML = 'Starting...';
            break;
        case 'disconnected':
            state.classList.add('idle');
            icon.textContent = '⚠️';
            label.innerHTML = 'Disconnected';
            break;
        case 'error':
            state.classList.add('idle');
            icon.textContent = '❌';
            label.innerHTML = text || 'Error';
            break;
        default:
            if (doneTimeout) { clearTimeout(doneTimeout); doneTimeout = null; }
            state.classList.add('idle');
            icon.textContent = '⏺';
            label.innerHTML = `Press and hold <kbd>${getHotkeyName()}</kbd> to record`;
            break;
    }
}

function updateConnectionBadge(status) {
    const el = elements.testConnection;
    if (!el) return;

    switch (status) {
        case 'idle':
        case 'recording':
        case 'sending':
        case 'processing':
        case 'improving':
        case 'typing':
            el.className = 'info-value connected';
            el.textContent = 'Connected';
            break;
        case 'connecting':
            el.className = 'info-value';
            el.textContent = 'Starting...';
            break;
        case 'disconnected':
            el.className = 'info-value disconnected';
            el.textContent = 'Disconnected';
            break;
        case 'error':
            el.className = 'info-value error';
            el.textContent = 'Error';
            break;
        default:
            el.className = 'info-value';
            el.textContent = status;
    }
}

function getStatusText(status) {
    switch (status) {
        case 'idle': return 'Ready';
        case 'recording': return 'Recording...';
        case 'sending': return 'Sending...';
        case 'processing': return 'Processing...';
        case 'typing': return 'Typing...';
        case 'connecting': return 'Starting...';
        case 'disconnected': return 'Disconnected';
        case 'error': return 'Error';
        default: return status;
    }
}

function getHotkeyName() {
    const hotkeyNames = {
        'fn': 'Fn',
        'ctrl': 'Ctrl',
        'ctrlright': 'Right Ctrl',
        'alt': 'Alt',
        'altright': 'Right Alt',
        'shift': 'Shift',
        'cmd': 'Cmd'
    };
    return hotkeyNames[config.hotkey] || config.hotkey;
}

function updateHotkeyHint() {
    const name = getHotkeyName();

    if (elements.testHotkey) {
        elements.testHotkey.innerHTML = `<kbd>${name}</kbd>`;
    }

    // Update hotkey-state label if in idle state
    if (elements.hotkeyState && elements.hotkeyState.classList.contains('idle')) {
        elements.hotkeyLabel.innerHTML = `Press and hold <kbd>${name}</kbd> to record`;
    }
}

function updateApiKeyHint() {
    const key = config.openai_api_key || '';
    const hint = document.getElementById('api-key-hint');
    if (hint) {
        if (key.length > 4) {
            hint.textContent = 'Key: ••••' + key.slice(-2);
            hint.style.display = '';
        } else {
            hint.style.display = 'none';
        }
    }
}

function updateTestMode() {
    if (elements.testMode) {
        elements.testMode.textContent = config.transcription_mode === 'openai' ? 'OpenAI API' : 'Local Whisper';
    }
}

function formatTimestamp(ts) {
    if (!ts) return '';
    const date = new Date(ts);
    return date.toLocaleTimeString();
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Poll backend for current status, transcriptions, and debug lines
function startStatusPolling() {
    if (statusPollTimer) return;
    statusPollTimer = setInterval(async () => {
        try {
            const data = await invoke('get_current_status');
            if (!data) return;

            // Update status if changed
            if (data.status) {
                const key = data.status + ':' + data.text;
                if (key !== lastPollStatus) {
                    // Don't let polling override "done" with "idle" — let the timeout handle it
                    const isDoneShowing = doneTimeout !== null;
                    const isIdleFromBackend = data.status === 'idle';
                    if (isDoneShowing && isIdleFromBackend) {
                        lastPollStatus = key;
                        // Skip UI update — "done" timer will revert to idle
                    } else {
                        lastPollStatus = key;
                        updateStatus(data.status, data.text);
                        updateConnectionBadge(data.status);
                    }
                }
            }

            // Update transcription if new one arrived
            if (data.transcription_count > lastPollTranscriptionCount) {
                lastPollTranscriptionCount = data.transcription_count;
                if (data.last_transcription) {
                    showTranscriptionText(data.last_transcription);
                }
            }

            // Fetch new debug lines if count changed
            if (data.debug_count > lastPollDebugCount) {
                const newLines = await invoke('get_debug_log');
                if (newLines && newLines.length > debugLines.length) {
                    const added = newLines.slice(debugLines.length);
                    debugLines = newLines;
                    // Append to DOM if log tab is visible
                    const debugTab = document.getElementById('log-tab');
                    if (debugTab.classList.contains('active')) {
                        added.forEach(line => appendDebugLineToDOM(line));
                    }
                }
                lastPollDebugCount = data.debug_count;
            }
        } catch (e) {
            // Ignore polling errors
        }
    }, 200);
}

function showTranscriptionText(text) {
    const el = elements.testResultText;
    if (!el) return;
    el.innerHTML = `<div class="text">${escapeHtml(text)}</div>`;
}

function stopStatusPolling() {
    if (statusPollTimer) {
        clearInterval(statusPollTimer);
        statusPollTimer = null;
    }
}

// ============================================================================
// Permissions check
// ============================================================================

async function checkPermissions() {
    try {
        const perms = await invoke('check_permissions');
        updatePermissionItem('perm-microphone', perms.microphone);
        updatePermissionItem('perm-accessibility', perms.accessibility);
        updatePermissionItem('perm-input_monitoring', perms.input_monitoring);

        if (perms.microphone && perms.accessibility && perms.input_monitoring) {
            elements.permissionsModal.classList.add('hidden');
        } else {
            elements.permissionsModal.classList.remove('hidden');
        }
    } catch (e) {
        console.error('Failed to check permissions:', e);
    }
}

function updatePermissionItem(elementId, granted) {
    const el = document.getElementById(elementId);
    if (!el) return;
    const icon = el.querySelector('.perm-icon');
    if (granted) {
        icon.textContent = '\u2705';
        el.classList.add('perm-granted');
        el.classList.remove('perm-denied');
    } else {
        icon.textContent = '\u274C';
        el.classList.add('perm-denied');
        el.classList.remove('perm-granted');
    }
}

function setupPermissionsListeners() {
    elements.openSettingsBtn.addEventListener('click', async () => {
        try {
            await invoke('open_privacy_settings');
        } catch (e) {
            console.error('Failed to open settings:', e);
        }
    });

    elements.checkAgainBtn.addEventListener('click', async () => {
        elements.checkAgainBtn.disabled = true;
        elements.checkAgainBtn.textContent = 'Reloading...';
        try {
            await invoke('restart_voice_typer');
        } catch (e) {
            console.error('Failed to restart voice-typer:', e);
        }
        await checkPermissions();
        elements.checkAgainBtn.disabled = false;
        elements.checkAgainBtn.textContent = 'Reload and Check';
    });
}

// ============================================================================
// Update overlay
// ============================================================================

function storeUpdateInfo(updateInfo) {
    _updateState.pendingInfo = updateInfo;
    _updateState.downloadUrl = updateInfo.download_url || null;
    _updateState.releaseUrl = updateInfo.release_url || updateInfo.url || null;
    _updateState.checksumsUrl = updateInfo.checksums_url || null;
    _updateState.assetFilename = updateInfo.asset_filename || null;
}

function showUpdateOverlay(updateInfo) {
    const info = updateInfo || _updateState.pendingInfo;
    if (!info) return;

    const overlay = document.getElementById('update-overlay');
    const currentVersionEl = document.getElementById('update-current-version');
    const newVersionEl = document.getElementById('update-new-version');

    if (!overlay) return;

    // Reset overlay state to initial conditions
    const progressArea = document.getElementById('update-progress');
    const progressText = document.getElementById('update-progress-text');
    const progressBar = document.getElementById('update-progress-bar');
    const installBtn = document.getElementById('update-install-btn');
    const laterBtn = document.getElementById('update-later-btn');

    if (progressArea) {
        progressArea.style.display = 'none';
    }
    if (progressText) {
        progressText.textContent = 'Downloading...';
    }
    if (progressBar) {
        progressBar.style.width = '0%';
    }
    if (installBtn) {
        installBtn.disabled = false;
        installBtn.textContent = 'Update';
        installBtn.onclick = installUpdate;
    }
    if (laterBtn) {
        laterBtn.style.display = '';
    }

    if (currentVersionEl) {
        const currentVersion = (info.current_version)
            ? info.current_version
            : (elements.appVersion ? elements.appVersion.textContent : '—');
        currentVersionEl.textContent = currentVersion || '—';
    }

    if (newVersionEl) {
        const latestVersion = info.latest_version || info.version || '—';
        newVersionEl.textContent = 'v' + latestVersion;
    }

    _updateState.downloadUrl = info.download_url || null;
    _updateState.releaseUrl = info.release_url || info.url || null;
    _updateState.checksumsUrl = info.checksums_url || null;
    _updateState.assetFilename = info.asset_filename || null;

    overlay.style.display = 'flex';
}

function dismissUpdateOverlay() {
    const overlay = document.getElementById('update-overlay');
    if (overlay) {
        overlay.style.display = 'none';
    }
}

async function installUpdate() {
    const btn = document.getElementById('update-install-btn');
    const progressArea = document.getElementById('update-progress');
    const progressText = document.getElementById('update-progress-text');
    const url = _updateState.downloadUrl;

    if (!url) {
        if (_updateState.releaseUrl) {
            console.log('No direct download URL, opening release page');
            if (progressText) {
                progressText.textContent = 'Please download the update manually from the release page.';
            }
            if (progressArea) {
                progressArea.style.display = '';
            }
            if (btn) {
                btn.textContent = 'Open Release Page';
                btn.disabled = false;
                btn.onclick = () => {
                    invoke('open_url', { url: _updateState.releaseUrl });
                };
            }
            const laterBtn = document.getElementById('update-later-btn');
            if (laterBtn) {
                laterBtn.style.display = '';
            }
        } else {
            console.error('No download URL or release URL available');
            if (progressText) {
                progressText.textContent = 'Error: No download URL available';
            }
            if (progressArea) {
                progressArea.style.display = '';
            }
            if (btn) {
                btn.disabled = false;
            }
        }
        return;
    }

    const laterBtn = document.getElementById('update-later-btn');

    if (btn) {
        btn.disabled = true;
    }

    if (laterBtn) {
        laterBtn.style.display = 'none';
    }

    if (progressArea) {
        progressArea.style.display = '';
    }

    if (progressText) {
        progressText.textContent = 'Downloading update...';
    }

    try {
        await invoke('install_update');
        if (progressText) {
            progressText.textContent = 'Update installed! Restarting...';
        }
    } catch (e) {
        console.error('Failed to install update:', e);
        if (progressText) {
            progressText.textContent = 'Error: ' + e;
        }
        if (btn) {
            btn.disabled = false;
        }
        if (laterBtn) {
            laterBtn.style.display = '';
        }
    }
}

// ============================================================================
// Version info and update checking
// ============================================================================

async function loadVersionInfo() {
    try {
        const info = await invoke('get_version_info');
        if (info && info.current_version && elements.appVersion) {
            elements.appVersion.textContent = 'v' + info.current_version;
        }
        const updateInfo = info && info.update_info;
        if (updateInfo && updateInfo.update_available && updateInfo.latest_version) {
            storeUpdateInfo(updateInfo);
            setUpdateStatusClickable('update-available', `New version available: v${updateInfo.latest_version}`);
        }
    } catch (e) {
        console.error('Failed to get version info:', e);
    }
}

async function checkForUpdate() {
    const btn = elements.checkUpdateBtn;
    if (!btn) return;

    btn.disabled = true;
    btn.textContent = 'Checking...';
    setUpdateStatus('', '');

    try {
        const result = await invoke('check_for_update');
        if (result && result.update_available) {
            const version = result.latest_version || result.version;
            storeUpdateInfo(result);
            setUpdateStatusClickable('update-available', `New version available: v${version}`);
        } else {
            setUpdateStatus('up-to-date', 'Up to date \u2713');
        }
    } catch (e) {
        console.error('Failed to check for update:', e);
        setUpdateStatus('check-failed', 'Check failed');
        setTimeout(() => setUpdateStatus('', ''), 3000);
    } finally {
        btn.disabled = false;
        btn.textContent = 'Check for updates';
    }
}

function setUpdateStatus(className, text) {
    const el = elements.appUpdateStatus;
    if (!el) return;
    el.className = 'app-update-status' + (className ? ' ' + className : '');
    el.textContent = text;
    el.onclick = null;
}

function setUpdateStatusClickable(className, text) {
    const el = elements.appUpdateStatus;
    if (!el) return;
    el.className = 'app-update-status clickable' + (className ? ' ' + className : '');
    el.textContent = text;
    el.onclick = function () {
        showUpdateOverlay();
    };
}

function setUpdateStatusHtml(className, text) {
    const el = elements.appUpdateStatus;
    if (!el) return;
    el.className = 'app-update-status' + (className ? ' ' + className : '');
    el.textContent = text;
}

function openGitHub() {
    window.open('https://github.com/alexmakeev/voice-keyboard', '_blank');
}
