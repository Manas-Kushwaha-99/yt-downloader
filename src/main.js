const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const urlInput = document.getElementById('urlInput');
const analyzeBtn = document.getElementById('analyzeBtn');
const downloadBtn = document.getElementById('downloadBtn');
const downloadBtnText = document.getElementById('downloadBtnText');
const browseBtn = document.getElementById('browseBtn');
const openFolderBtn = document.getElementById('openFolderBtn');
const updateBtn = document.getElementById('updateBtn');
const minimizeBtn = document.getElementById('minimizeBtn');
const closeBtn = document.getElementById('closeBtn');
const updateCloseBtn = document.getElementById('updateCloseBtn');

const versionBadge = document.getElementById('versionBadge');
const versionText = document.getElementById('versionText');
const versionDot = document.querySelector('.version-dot');

const infoSection = document.getElementById('infoSection');
const videoTitle = document.getElementById('videoTitle');
const resolutionOptions = document.getElementById('resolutionOptions');
const outputPath = document.getElementById('outputPath');

const progressSection = document.getElementById('progressSection');
const progressFilename = document.getElementById('progressFilename');
const progressPercent = document.getElementById('progressPercent');
const progressBarFill = document.getElementById('progressBarFill');
const progressSize = document.getElementById('progressSize');
const progressSpeed = document.getElementById('progressSpeed');
const progressEta = document.getElementById('progressEta');

const toast = document.getElementById('toast');
const updateOverlay = document.getElementById('updateOverlay');
const updateLog = document.getElementById('updateLog');

const appWindow = getCurrentWindow();

let state = {
  ytdlpVersion: null,
  videoInfo: null,
  selectedFormat: 'video',
  selectedHeight: null,
  outputDir: '',
  isDownloading: false,
};

minimizeBtn.addEventListener('click', () => appWindow.minimize());
closeBtn.addEventListener('click', () => appWindow.close());

function showToast(message, type = '') {
  toast.textContent = message;
  toast.className = 'toast ' + type + ' show';
  const duration = type === 'error' || type === 'warning' ? 4000 : 3000;
  clearTimeout(toast._timeout);
  toast._timeout = setTimeout(() => toast.classList.remove('show'), duration);
}

async function checkYtdlp() {
  try {
    const version = await invoke('check_ytdlp');
    state.ytdlpVersion = version;
    versionText.textContent = 'yt-dlp v' + version;
    versionDot.classList.add('active');
  } catch (e) {
    versionText.textContent = 'yt-dlp not found';
    versionDot.classList.remove('active');
    showToast('yt-dlp is not installed. Install it first.', 'error');
  }
}

analyzeBtn.addEventListener('click', async () => {
  const url = urlInput.value.trim();
  if (!url) {
    showToast('Please enter a YouTube URL', 'warning');
    return;
  }

  analyzeBtn.disabled = true;
  analyzeBtn.querySelector('span').textContent = 'Analyzing...';

  try {
    const info = await invoke('fetch_formats', { url });
    state.videoInfo = info;
    videoTitle.textContent = info.title;
    renderResolutionChips();
    infoSection.style.display = 'flex';
    updateDownloadButton();
  } catch (e) {
    const message = String(e).replace(/^\[.*?\]\s*/, '');
    showToast(message || 'Failed to analyze URL', 'error');
    infoSection.style.display = 'none';
    state.videoInfo = null;
    updateDownloadButton();
  } finally {
    analyzeBtn.disabled = false;
    analyzeBtn.querySelector('span').textContent = 'Analyze';
  }
});

function renderResolutionChips() {
  resolutionOptions.innerHTML = '';
  const qualities = state.videoInfo.available_qualities;
  const isVertical = state.videoInfo.is_vertical;

  if (state.selectedFormat === 'audio') {
    resolutionOptions.style.display = 'none';
    state.selectedHeight = null;
    return;
  }

  resolutionOptions.style.display = 'flex';

  if (qualities.length === 0) {
    resolutionOptions.innerHTML = '<span style="font-size:12px;color:var(--text-muted);">No resolution info</span>';
    return;
  }

  const suffix = isVertical ? ' (Shorts)' : '';
  const nameMap = { 4320: '8K', 2160: '4K', 1440: '1440p', 1080: '1080p', 720: '720p', 480: '480p', 360: '360p', 240: '240p', 144: '144p' };

  qualities.forEach(q => {
    const label = (nameMap[q] || q + 'p') + suffix;

    const chip = document.createElement('button');
    chip.className = 'resolution-chip';
    chip.textContent = label;
    chip.dataset.quality = q;

    if (state.selectedHeight === q || (state.selectedHeight === null && q === state.videoInfo.max_quality)) {
      chip.classList.add('selected');
      state.selectedHeight = q;
    }

    chip.addEventListener('click', () => {
      document.querySelectorAll('.resolution-chip').forEach(c => c.classList.remove('selected'));
      chip.classList.add('selected');
      state.selectedHeight = parseInt(chip.dataset.quality);
      updateDownloadButton();
    });

    resolutionOptions.appendChild(chip);
  });
}

document.getElementById('videoToggle').addEventListener('click', () => {
  if (state.selectedFormat === 'video') return;
  state.selectedFormat = 'video';
  document.getElementById('videoToggle').classList.add('active');
  document.getElementById('audioToggle').classList.remove('active');
  if (state.videoInfo) renderResolutionChips();
  updateDownloadButton();
});

document.getElementById('audioToggle').addEventListener('click', () => {
  if (state.selectedFormat === 'audio') return;
  state.selectedFormat = 'audio';
  document.getElementById('audioToggle').classList.add('active');
  document.getElementById('videoToggle').classList.remove('active');
  if (state.videoInfo) renderResolutionChips();
  updateDownloadButton();
});

browseBtn.addEventListener('click', async () => {
  try {
    const folder = await invoke('pick_folder');
    if (folder) {
      state.outputDir = folder;
      outputPath.textContent = folder;
      outputPath.classList.add('selected');
      openFolderBtn.disabled = false;
      updateDownloadButton();
    }
  } catch (e) {
    showToast('Failed to pick folder', 'error');
  }
});

openFolderBtn.addEventListener('click', async () => {
  if (!state.outputDir) return;
  try {
    await invoke('open_folder', { path: state.outputDir });
  } catch (e) {
    showToast('Failed to open folder', 'error');
  }
});

function updateDownloadButton() {
  const canDownload =
    state.videoInfo &&
    state.outputDir &&
    !state.isDownloading &&
    (state.selectedFormat === 'audio' || state.selectedHeight !== null);
  downloadBtn.disabled = !canDownload;
}

downloadBtn.addEventListener('click', async () => {
  if (state.isDownloading) return;

  if (!state.videoInfo || !state.outputDir) return;

  let quality = String(state.selectedHeight || 'best');

  if (state.selectedFormat === 'video' && state.selectedHeight > state.videoInfo.max_quality) {
    quality = String(state.videoInfo.max_quality);
    const label = state.selectedHeight >= 2160 ? '4K' : state.selectedHeight + 'p';
    showToast(label + ' not available, downloading in best quality available', 'warning');
  }

  state.isDownloading = true;
  downloadBtn.classList.add('loading');
  downloadBtnText.textContent = 'Starting download...';
  downloadBtn.disabled = true;
  progressSection.style.display = 'flex';
  progressFilename.textContent = 'Preparing...';
  progressPercent.textContent = '0%';
  progressBarFill.style.width = '0%';
  progressSize.textContent = '--';
  progressSpeed.textContent = '--';
  progressEta.textContent = '--';

  try {
    await invoke('start_download', {
      url: urlInput.value.trim(),
      quality: quality,
      outputDir: state.outputDir,
      audioOnly: state.selectedFormat === 'audio',
      isVertical: state.videoInfo.is_vertical,
    });
  } catch (e) {
    // Error is already handled via events
  }
});

function toDecimal(bytesStr) {
  const num = parseFloat(bytesStr);
  const unit = bytesStr.replace(/[\d.\s]/g, '');
  const bytes = unit === 'KiB' ? num * 1024 : unit === 'MiB' ? num * 1048576 : unit === 'GiB' ? num * 1073741824 : num;
  if (bytes >= 1000000000) return (bytes / 1000000000).toFixed(2) + ' GB';
  if (bytes >= 1000000) return (bytes / 1000000).toFixed(2) + ' MB';
  if (bytes >= 1000) return (bytes / 1000).toFixed(2) + ' KB';
  return bytes.toFixed(0) + ' B';
}

listen('download-progress', (event) => {
  const data = event.payload;
  progressFilename.textContent = data.filename || 'Downloading...';
  progressPercent.textContent = data.percent.toFixed(1) + '%';
  progressBarFill.style.width = data.percent + '%';
  if (data.total_size) {
    const sizeVal = parseFloat(data.total_size);
    const sizeUnit = data.total_size.replace(/[\d.\s]/g, '');
    const prevSize = progressSize.textContent;
    const prevVal = parseFloat(prevSize) || 0;
    const prevUnit = prevSize.replace(/[\d.\s]/g, '');
    const toBytes = (v, u) => {
      const clean = u.trim();
      if (clean === 'MiB' || clean === 'MB') return v * 1048576;
      if (clean === 'KiB' || clean === 'KB') return v * 1024;
      if (clean === 'GiB' || clean === 'GB') return v * 1073741824;
      return 0;
    };
    const newDec = toDecimal(data.total_size);
    const newBytes = toBytes(sizeVal, sizeUnit);
    const prevBytes = toBytes(prevVal, prevUnit);
    if (newBytes >= prevBytes) {
      progressSize.textContent = newDec;
    }
  }
  if (data.speed) progressSpeed.textContent = data.speed;
  if (data.eta) progressEta.textContent = data.eta;
});

listen('download-complete', (event) => {
  state.isDownloading = false;
  downloadBtn.classList.remove('loading');
  downloadBtnText.textContent = 'Download';
  updateDownloadButton();
  progressPercent.textContent = '100%';
  progressBarFill.style.width = '100%';
  if (event.payload.filename && event.payload.filename !== 'Download finished') {
    progressFilename.textContent = event.payload.filename;
  }
  if (event.payload.file_size) {
    progressSize.textContent = event.payload.file_size;
  }
  showToast('Download complete!', 'success');
});

listen('download-error', (event) => {
  const data = event.payload;
  state.isDownloading = false;
  downloadBtn.classList.remove('loading');
  downloadBtnText.textContent = 'Download';
  updateDownloadButton();
  showToast(data.message, 'error');
});

listen('update-line', (event) => {
  const line = document.createElement('div');
  line.textContent = event.payload.line;
  updateLog.appendChild(line);
  updateLog.scrollTop = updateLog.scrollHeight;
});

updateBtn.addEventListener('click', () => {
  updateOverlay.style.display = 'flex';
  updateLog.innerHTML = '';
  invoke('update_ytdlp')
    .then((result) => {
      const line = document.createElement('div');
      line.textContent = result;
      line.style.color = 'var(--success)';
      updateLog.appendChild(line);
      updateLog.scrollTop = updateLog.scrollHeight;
      checkYtdlp();
    })
    .catch((e) => {
      const line = document.createElement('div');
      line.textContent = String(e);
      line.style.color = 'var(--danger)';
      updateLog.appendChild(line);
    });
});

updateCloseBtn.addEventListener('click', () => {
  updateOverlay.style.display = 'none';
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !state.isDownloading) analyzeBtn.click();
  if (e.key === 'Escape' && updateOverlay.style.display === 'flex') {
    updateOverlay.style.display = 'none';
  }
});

checkYtdlp();
