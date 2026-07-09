// Voice Keyboard UI Application

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// State
let transcriptions = [];
let debugLines = [];
const MAX_DEBUG_LINES = 2000;
let statusPollTimer = null;
let lastPollStatus = null;
let permissionsPollTimer = null;
// Tracks whether the previous checkPermissions() tick saw every required
// permission granted, so we can detect the false→true transition (see
// checkPermissions below) and restart the sidecar immediately instead of
// waiting for its own internal grab() retry loop's next tick.
let lastAllPermissionsGranted = false;
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
    language: 'auto',
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
    { code: 'ar', name: 'Arabic' },
    { code: 'zh', name: 'Chinese' },
    { code: 'cs', name: 'Czech' },
    { code: 'nl', name: 'Dutch' },
    { code: 'en', name: 'English' },
    { code: 'fr', name: 'French' },
    { code: 'de', name: 'German' },
    { code: 'it', name: 'Italian' },
    { code: 'ja', name: 'Japanese' },
    { code: 'ko', name: 'Korean' },
    { code: 'pl', name: 'Polish' },
    { code: 'pt', name: 'Portuguese' },
    { code: 'ru', name: 'Russian' },
    { code: 'es', name: 'Spanish' },
    { code: 'tr', name: 'Turkish' },
    { code: 'uk', name: 'Ukrainian' },
];

// DOM Elements
let elements = {};

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
    cacheElements();
    setupSettingsPanel();
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
    startPermissionsPolling();
    checkApiKeyRequired();
});

function cacheElements() {
    elements = {
        // Header
        headerVersion: document.getElementById('header-version'),
        connectionBadge: document.getElementById('connection-badge'),
        connectionBadgeIcon: document.getElementById('connection-badge-icon'),
        connectionBadgeLabel: document.getElementById('connection-badge-label'),
        gearBtn: document.getElementById('gear-btn'),
        // Main screen
        mainHotkeyName: document.getElementById('main-hotkey-name'),
        captionHotkeyName: document.getElementById('caption-hotkey-name'),
        recordBtn: document.getElementById('record-btn'),
        recordBtnLabel: document.getElementById('record-btn-label'),
        testResultText: document.getElementById('test-result-text'),
        testResultCaption: document.getElementById('test-result-caption'),
        // Settings overlay
        settingsOverlay: document.getElementById('settings-overlay'),
        settingsCloseBtn: document.getElementById('settings-close-btn'),
        showLogBtn: document.getElementById('show-log-btn'),
        logDisclosureLabel: document.getElementById('log-disclosure-label'),
        logPanelSection: document.getElementById('log-panel-section'),
        // Log (debug) panel
        debugLog: document.getElementById('debug-log'),
        debugLogContainer: document.querySelector('.debug-log-container'),
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
        // App info
        appVersion: document.getElementById('app-version'),
        appUpdateStatus: document.getElementById('app-update-status'),
        settingsUpdateBtn: document.getElementById('settings-update-btn'),
        // Permissions modal
        permissionsModal: document.getElementById('permissions-modal'),
        openMicrophoneBtn: document.getElementById('open-microphone-settings-btn'),
        openAccessibilityBtn: document.getElementById('open-accessibility-settings-btn'),
        checkAgainBtn: document.getElementById('check-again-btn'),
        // Report modal
        reportModal: document.getElementById('report-modal'),
        cancelReportBtn: document.getElementById('cancel-report'),
        createReportBtn: document.getElementById('create-report'),
        modeCards: document.querySelectorAll('.mode-card'),
    };
}

// Settings fields update `config` and set this flag while the panel is
// open, instead of saving+reloading on every change (see saveSettings()).
// Deferring the actual save+reload until the panel closes avoids a
// save+reload round-trip -- and the UI stutter that comes with it -- for
// every single field edit.
let settingsDirty = false;

function openSettingsPanel() {
    elements.settingsOverlay.classList.remove('hidden');
    elements.gearBtn.classList.add('active');
}

function closeSettingsPanel() {
    elements.settingsOverlay.classList.add('hidden');
    elements.gearBtn.classList.remove('active');
    // Apply all accumulated changes in one shot, only if something actually
    // changed -- avoids a spurious save+reload when the panel is opened and
    // closed without editing anything.
    if (settingsDirty) {
        settingsDirty = false;
        saveSettings();
    }
}

function expandLogPanel() {
    elements.logPanelSection.classList.remove('hidden');
    elements.showLogBtn.classList.add('expanded');
    if (elements.logDisclosureLabel) elements.logDisclosureLabel.textContent = 'Hide Log';
    renderDebugLog();
    // Jump straight to the most recent entries when the panel opens.
    elements.debugLogContainer.scrollTop = elements.debugLogContainer.scrollHeight;
}

function collapseLogPanel() {
    elements.logPanelSection.classList.add('hidden');
    elements.showLogBtn.classList.remove('expanded');
    if (elements.logDisclosureLabel) elements.logDisclosureLabel.textContent = 'Show Log';
}

function setupSettingsPanel() {
    elements.gearBtn.addEventListener('click', () => {
        if (elements.settingsOverlay.classList.contains('hidden')) {
            openSettingsPanel();
        } else {
            closeSettingsPanel();
        }
    });
    elements.settingsCloseBtn.addEventListener('click', closeSettingsPanel);
    // Clicking the darkened backdrop (outside the slide-over panel itself)
    // should close it, like any standard overlay/modal. Only close when the
    // click target IS the overlay element -- clicks that start inside
    // .settings-panel and bubble up to the overlay must NOT close it.
    elements.settingsOverlay.addEventListener('click', (e) => {
        if (e.target === elements.settingsOverlay) {
            closeSettingsPanel();
        }
    });
    // "Show Log" / "Hide Log" collapsible section header.
    elements.showLogBtn.addEventListener('click', () => {
        if (elements.logPanelSection.classList.contains('hidden')) {
            expandLogPanel();
        } else {
            collapseLogPanel();
        }
    });
}

// Text-input fields (API key/URL) save on blur rather than on every
// keystroke, to avoid a save+reload round-trip per character while typing.
// Dropdowns/checkboxes/radios save immediately on change/click, since those
// are discrete, deliberate actions rather than continuous typing.
function setupEventListeners() {
    // Record button: push-to-talk via mouse (for testing without working hotkey)
    const recBtn = elements.recordBtn;
    if (recBtn) {
        const stopRecording = async () => {
            try { await invoke('test_recording_stop'); } catch (e) { console.error('test_recording_stop failed:', e); }
        };
        recBtn.addEventListener('mousedown', async (e) => {
            e.preventDefault();
            try { await invoke('test_recording_start'); } catch (e) { console.error('test_recording_start failed:', e); }
        });
        recBtn.addEventListener('mouseup', stopRecording);
        recBtn.addEventListener('mouseleave', stopRecording);
        // Touch support (optional, desktop-primary)
        recBtn.addEventListener('touchstart', async (e) => {
            e.preventDefault();
            try { await invoke('test_recording_start'); } catch (e) { console.error('test_recording_start failed:', e); }
        }, { passive: false });
        recBtn.addEventListener('touchend', stopRecording);
    }

    // Transcription result textarea is user-editable; keep the caption's
    // visibility in sync if the user manually clears/edits its content.
    if (elements.testResultText) {
        elements.testResultText.addEventListener('input', (e) => {
            if (elements.testResultCaption) {
                elements.testResultCaption.classList.toggle('hidden', !e.target.value);
            }
        });
    }

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

    // Settings changes -- each field only updates the in-memory `config` and
    // marks settingsDirty while the panel is open. The actual save+reload
    // (saveSettings()) fires once, when the panel closes (see
    // closeSettingsPanel()) -- saving on every field change caused
    // noticeable UI lag/stutter while adjusting settings.
    elements.languageSelect.addEventListener('change', (e) => {
        config.language = e.target.value;
        settingsDirty = true;
    });

    elements.hotkeySelect.addEventListener('change', (e) => {
        config.hotkey = e.target.value;
        updateHotkeyHint();
        settingsDirty = true;
    });

    elements.inputMethodSelect.addEventListener('change', (e) => {
        config.input_method = e.target.value;
        settingsDirty = true;
    });

    elements.openaiKeyInput.addEventListener('input', (e) => {
        config.openai_api_key = e.target.value;
        updateApiKeyHint();
        checkApiKeyRequired();
        settingsDirty = true;
    });

    elements.openaiUrlInput.addEventListener('input', (e) => {
        config.openai_api_url = e.target.value;
        settingsDirty = true;
    });

    elements.soundEnabled.addEventListener('change', (e) => {
        config.sound_enabled = e.target.checked;
        settingsDirty = true;
    });

    elements.audioDeviceSelect.addEventListener('change', (e) => {
        config.audio_device = e.target.value;
        settingsDirty = true;
    });

    elements.lowerVolume.addEventListener('change', (e) => {
        config.lower_volume_on_record = e.target.checked;
        settingsDirty = true;
    });

    // Minimum Duration is a plain text input (no native number spinners) --
    // strip non-digit characters as the user types, and clamp to the valid
    // range once they're done editing (on blur), rather than on every
    // keystroke, so e.g. typing "1000" isn't clamped to "100" mid-edit.
    elements.minRecordingMs.addEventListener('input', (e) => {
        const digitsOnly = e.target.value.replace(/[^0-9]/g, '');
        if (digitsOnly !== e.target.value) e.target.value = digitsOnly;
        settingsDirty = true;
    });

    elements.minRecordingMs.addEventListener('blur', (e) => {
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
            settingsDirty = true;
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
        updateConnectionBadge(payload.status, payload.text);
    });

    // Listen for new transcriptions
    await listen('transcription', (event) => {
        addTranscription(event.payload);
        // The record circle's sending->idle transition normally comes from a
        // 'status-update' event whose text is heuristically matched against
        // the voice-typer sidecar's stdout (see extract_status() in
        // src-tauri/src/main.rs). That heuristic can miss a terminal status
        // for some backend code paths, leaving the circle stuck on
        // "Sending..." forever even though the result already arrived here.
        // The arrival of a transcription result IS, by definition, "done" --
        // so treat it as a direct, authoritative signal to reset the circle,
        // instead of depending solely on the separate stdout-derived status.
        // (status-update's idle/done handling is still needed too, for the
        // "no speech detected" / recording-too-short case where no
        // transcription event is ever emitted.)
        setRecordCircleState('idle');
    });

    // Listen for debug log lines
    await listen('debug-log', (event) => {
        addDebugLine(event.payload);
    });

    // Listen for tray "Settings" menu item -- opens the Settings slide-over panel.
    await listen('open-settings', () => {
        openSettingsPanel();
    });

    // Listen for tray "Open" menu item -- forces the Settings panel closed so
    // the window lands on the home screen instead of a stale Settings state.
    await listen('close-settings', () => {
        closeSettingsPanel();
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
    // Note: preprompt_default/1/2/3 ("Enhance messages") are intentionally
    // NOT read into any UI control -- that section's UI has been removed.
    // The fields still round-trip through `config` on save so any existing
    // values on disk are preserved untouched.
    updateHotkeyHint();

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

// Sets the transcription result textarea's content and toggles the
// explanatory caption below it based on whether it's non-empty -- mirrors
// the design mockup's mockupSetTranscription helper, wired to real state.
function setTranscriptionDisplay(text) {
    const el = elements.testResultText;
    const caption = elements.testResultCaption;
    if (!el) return;
    el.value = text || '';
    if (caption) caption.classList.toggle('hidden', !text);
}

function renderTranscriptions() {
    if (transcriptions.length === 0) {
        setTranscriptionDisplay('');
        return;
    }

    const last = transcriptions[transcriptions.length - 1];
    setTranscriptionDisplay(last.text);
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

    // Only render if the log panel is currently revealed inside Settings
    const logPanel = elements.logPanelSection;
    if (logPanel && !logPanel.classList.contains('hidden')) {
        appendDebugLineToDOM(line);
    }
}

// The standalone "auto-scroll" toggle was removed along with "clear" (log
// panel filter chips now span the full row on their own) -- the log panel
// always tails to the bottom instead, both on initial render and as new
// lines stream in, for as long as it's expanded (callers only invoke these
// while the panel is visible; see addDebugLine()/expandLogPanel()).
function renderDebugLog() {
    const filtered = debugLines.filter(line => debugFilters[line.category] !== false);
    elements.debugLog.innerHTML = filtered.map(line => formatDebugLine(line)).join('');
    elements.debugLogContainer.scrollTop = elements.debugLogContainer.scrollHeight;
}

function appendDebugLineToDOM(line) {
    if (debugFilters[line.category] === false) return;

    const html = formatDebugLine(line);
    elements.debugLog.insertAdjacentHTML('beforeend', html);
    elements.debugLogContainer.scrollTop = elements.debugLogContainer.scrollHeight;
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
            settingsDirty = true;
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

// Replaces the old "Save & Reload" button: called once, when the Settings
// panel closes (see closeSettingsPanel()), instead of on every individual
// field change -- saving+reloading per keystroke/click caused noticeable UI
// lag while adjusting settings. Reads current values straight from the DOM,
// so it's safe to call at any time. preprompt_default/1/2/3 ("Enhance
// messages") are deliberately NOT touched here -- their UI was removed, so
// whatever value loadConfig() pulled in from disk is round-tripped
// unchanged.
async function saveSettings() {
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
        await invoke('save_config', { config });
    } catch (e) {
        console.error('Failed to save config:', e);
        alert('Failed to save settings: ' + e);
    }
}

// Drives the main screen's record circle (idle / listening / sending), the
// 3-state visual introduced by the redesign. The circle is a simplified
// summary -- the finer-grained backend statuses (sending, processing,
// improving, typing) all collapse into the single "sending" visual state;
// the connection badge and log panel still carry the detailed status.
function setRecordCircleState(state, labelText) {
    const btn = elements.recordBtn;
    const label = elements.recordBtnLabel;
    if (!btn || !label) return;
    btn.classList.remove('state-idle', 'state-listening', 'state-sending');
    btn.classList.add('state-' + state);
    if (state === 'idle') {
        label.innerHTML = `Hold <kbd>${getHotkeyName()}</kbd>`;
    } else {
        label.textContent = labelText || '';
    }
}

function updateStatus(status, text) {
    if (!elements.recordBtn) return;

    // Clear done timer if a new active status arrives
    if (status !== 'done' && status !== 'idle' && doneTimeout) {
        clearTimeout(doneTimeout);
        doneTimeout = null;
    }

    switch (status) {
        case 'recording':
            setRecordCircleState('listening', 'Listening...');
            break;
        case 'sending':
        case 'processing':
        case 'improving':
        case 'typing':
            setRecordCircleState('sending', 'Sending...');
            break;
        case 'done':
            // Skip "Done" display, go directly to idle
            if (doneTimeout) clearTimeout(doneTimeout);
            doneTimeout = null;
            setRecordCircleState('idle');
            break;
        case 'connecting':
        case 'disconnected':
        case 'error':
            setRecordCircleState('idle');
            break;
        default:
            if (doneTimeout) { clearTimeout(doneTimeout); doneTimeout = null; }
            setRecordCircleState('idle');
            break;
    }
}

// Drives the header's connection-status pill. The mockup defines exactly
// three visual states (connected/connecting/error); backend statuses that
// aren't a literal connection state (recording/sending/etc.) are treated as
// "connected" since the sidecar is clearly up and running, and
// 'disconnected' is folded into 'error' since both mean "not usable".
function updateConnectionBadge(status) {
    const badge = elements.connectionBadge;
    const icon = elements.connectionBadgeIcon;
    const label = elements.connectionBadgeLabel;
    if (!badge || !icon || !label) return;

    let state;
    switch (status) {
        case 'connecting':
            state = 'connecting';
            break;
        case 'disconnected':
        case 'error':
            state = 'error';
            break;
        default:
            state = 'connected';
    }

    badge.classList.remove('state-connected', 'state-connecting', 'state-error');
    badge.classList.add('state-' + state);

    if (state === 'connected') {
        icon.innerHTML = '&#10003;';
        label.textContent = 'Connected';
    } else if (state === 'connecting') {
        icon.innerHTML = '<span class="connection-spinner"></span>';
        label.textContent = 'Connecting';
    } else {
        icon.innerHTML = '&#10005;';
        label.textContent = 'Error';
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

    if (elements.mainHotkeyName) {
        elements.mainHotkeyName.textContent = name;
    }
    if (elements.captionHotkeyName) {
        elements.captionHotkeyName.textContent = name;
    }

    // Update the record circle's idle label, but only while it's actually
    // idle -- don't clobber "Listening..."/"Sending..." mid-recording.
    if (elements.recordBtn && elements.recordBtn.classList.contains('state-idle')) {
        elements.recordBtnLabel.innerHTML = `Hold <kbd>${name}</kbd>`;
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

function checkApiKeyRequired() {
    const isOpenai = config.transcription_mode === 'openai';
    const keyEmpty = !(config.openai_api_key && config.openai_api_key.trim());
    const keyInput = elements.openaiKeyInput;

    if (isOpenai && keyEmpty) {
        // Open the Settings panel so the user can see the API key field
        openSettingsPanel();

        // Highlight the API key field
        if (keyInput) {
            keyInput.classList.add('input-error');
            keyInput.focus();
        }

        // Show required hint (reuse api-key-hint element or add one)
        let reqHint = document.getElementById('api-key-required-hint');
        if (!reqHint && keyInput) {
            reqHint = document.createElement('span');
            reqHint.id = 'api-key-required-hint';
            reqHint.className = 'api-key-required-hint';
            reqHint.textContent = 'API key is required';
            keyInput.parentNode.insertBefore(reqHint, keyInput.nextSibling);
        }
    } else {
        // Remove highlight if key is now set
        if (keyInput) keyInput.classList.remove('input-error');
        const reqHint = document.getElementById('api-key-required-hint');
        if (reqHint) reqHint.remove();
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
                        updateConnectionBadge(data.status, data.text);
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
                    // Append to DOM if the log panel is currently revealed
                    const logPanel = elements.logPanelSection;
                    if (logPanel && !logPanel.classList.contains('hidden')) {
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
    setTranscriptionDisplay(text);
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

        // If the OS hasn't asked the user yet, trigger the system dialog now
        // so they see it at startup rather than on first hotkey press
        if (perms.microphone_status === 'not_determined') {
            await invoke('request_microphone_permission');
            // Re-fetch after the dialog resolves
            return checkPermissions();
        }

        updatePermissionItem('perm-microphone', perms.microphone);
        updatePermissionItem('perm-accessibility', perms.accessibility);

        // Each row's "Open Settings" deep-link is only useful while that
        // specific permission is still missing — hide it once granted.
        if (elements.openMicrophoneBtn) {
            elements.openMicrophoneBtn.classList.toggle('hidden', !!perms.microphone);
        }
        if (elements.openAccessibilityBtn) {
            elements.openAccessibilityBtn.classList.toggle('hidden', !!perms.accessibility);
        }

        const micBlocked = perms.microphone_status === 'denied' || perms.microphone_status === 'restricted';
        if (micBlocked) {
            showMicDeniedBanner();
        } else {
            hideMicDeniedBanner();
        }

        const allGranted = !!(perms.microphone && perms.accessibility);
        if (allGranted) {
            elements.permissionsModal.classList.add('hidden');
        } else {
            elements.permissionsModal.classList.remove('hidden');
        }

        // Snappier UX than waiting for the sidecar's own internal grab() retry
        // loop: the moment permissions transition from "not all granted" to
        // "all granted" (e.g. the user just approved Input Monitoring in System
        // Settings), restart the sidecar right away so the hotkey listener picks
        // up a fresh grab() attempt immediately instead of on its next backoff
        // tick. Only fires on the false→true transition — not on every poll —
        // so an already-working hotkey listener isn't needlessly bounced.
        if (allGranted && !lastAllPermissionsGranted) {
            // Flip the flag before restarting so the recursive checkPermissions()
            // call below (and any concurrent poll tick) doesn't see false→true
            // again and trigger a duplicate restart.
            lastAllPermissionsGranted = true;
            try {
                await invoke('restart_voice_typer');
            } catch (e) {
                console.error('Failed to restart voice-typer after permissions granted:', e);
            }
            await checkPermissions();
            return;
        }
        lastAllPermissionsGranted = allGranted;
    } catch (e) {
        console.error('Failed to check permissions:', e);
    }
}

function showMicDeniedBanner() {
    let banner = document.getElementById('mic-denied-banner');
    if (!banner) {
        banner = document.createElement('div');
        banner.id = 'mic-denied-banner';
        banner.style.cssText = 'background:#c0392b;color:#fff;padding:10px 16px;text-align:center;font-size:13px;';
        banner.innerHTML = 'Voice Keyboard needs microphone access. Enable microphone access in your system privacy settings and make sure an input device is available. ';
        const button = document.createElement('button');
        button.textContent = 'Open Settings';
        button.style.cssText = 'margin-left:8px;padding:2px 8px;cursor:pointer;';
        button.addEventListener('click', async () => {
            try {
                await invoke('open_privacy_settings');
            } catch (e) {
                console.error('Failed to open settings:', e);
            }
        });
        banner.appendChild(button);
        document.body.insertBefore(banner, document.body.firstChild);
    }
    banner.style.display = '';
}

function hideMicDeniedBanner() {
    const banner = document.getElementById('mic-denied-banner');
    if (banner) banner.style.display = 'none';
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
    // Microphone and Input Monitoring both trigger the real sidecar
    // permission-request preflight ("Grant") rather than just deep-linking to
    // System Settings: Microphone can show a genuine native OS dialog on
    // first request, and even though Input Monitoring's dialog is unreliable
    // from the sidecar's headless subprocess, requesting it is still the
    // more correct primary action than jumping straight to Settings.
    // Re-requesting an already-granted permission is a harmless no-op, so
    // reusing the same full preflight for both rows is safe.
    elements.openMicrophoneBtn.addEventListener('click', async () => {
        elements.openMicrophoneBtn.disabled = true;
        try {
            await invoke('request_permissions');
        } catch (e) {
            console.error('Failed to request Microphone permission:', e);
        }
        await checkPermissions();
        elements.openMicrophoneBtn.disabled = false;
    });

    elements.openAccessibilityBtn.addEventListener('click', async () => {
        try {
            await invoke('open_accessibility_settings');
        } catch (e) {
            console.error('Failed to open Accessibility settings:', e);
        }
    });

    elements.checkAgainBtn.addEventListener('click', async () => {
        elements.checkAgainBtn.disabled = true;
        elements.checkAgainBtn.textContent = 'Restarting...';
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

/// Poll permission status every 2s so checkmarks/banner reflect grants made
/// in System Settings live, without requiring the user to manually restart
/// anything. Cheap enough to run for the app's whole lifetime: each tick is
/// just two fast local TCC status reads (wrapper) plus one short-lived
/// `voice-typer --check-permissions` subprocess call (sidecar), no dialogs,
/// no audio/input side effects.
function startPermissionsPolling() {
    if (permissionsPollTimer) return;
    permissionsPollTimer = setInterval(() => {
        checkPermissions();
    }, 2000);
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
        if (info && info.current_version) {
            const versionText = 'v' + info.current_version;
            if (elements.appVersion) elements.appVersion.textContent = versionText;
            if (elements.headerVersion) elements.headerVersion.textContent = versionText;
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
        const reason = (typeof e === 'string' ? e : (e && e.message ? e.message : String(e))) || 'Unknown error';
        setUpdateStatus('check-failed', 'Check failed: ' + reason);
        setTimeout(() => setUpdateStatus('', ''), 6000);
    } finally {
        btn.disabled = false;
        btn.textContent = 'Check for updates';
    }
}

// Default/no-update state: plain status text, no "Update" button.
function setUpdateStatus(className, text) {
    const el = elements.appUpdateStatus;
    if (el) {
        el.className = 'app-update-status' + (className ? ' ' + className : '');
        el.textContent = text;
        el.onclick = null;
    }
    if (elements.settingsUpdateBtn) {
        elements.settingsUpdateBtn.classList.add('hidden');
    }
}

// Update-available state: shows the current-vs-available comparison text
// and reveals the orange "Update" button (replaces the old "Check for
// Updates" button, which only makes sense once an update is known).
function setUpdateStatusClickable(className, text) {
    const el = elements.appUpdateStatus;
    if (el) {
        el.className = 'app-update-status clickable' + (className ? ' ' + className : '');
        el.textContent = text;
        el.onclick = function () {
            showUpdateOverlay();
        };
    }
    if (elements.settingsUpdateBtn) {
        elements.settingsUpdateBtn.classList.remove('hidden');
    }
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
