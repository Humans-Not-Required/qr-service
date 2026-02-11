import { useState, useCallback, useEffect, useRef } from 'react'
import { generateQR, decodeQR, generateFromTemplate, createTrackedQR, getTrackedStats, deleteTrackedQR, healthCheck } from './api'
import './App.css'

const STYLES = ['square', 'rounded', 'dots'];
const FORMATS = ['png', 'svg'];
const EC_LEVELS = ['L', 'M', 'Q', 'H'];
const TEMPLATES = ['wifi', 'vcard', 'url'];

// ========== Toast Hook ==========

function useToast(duration = 2000) {
  const [toast, setToast] = useState(null);
  const timerRef = useRef(null);

  const showToast = useCallback((message, type = 'success') => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setToast({ message, type });
    timerRef.current = setTimeout(() => setToast(null), duration);
  }, [duration]);

  return { toast, showToast };
}

function Toast({ toast }) {
  if (!toast) return null;
  return (
    <div className={`toast ${toast.type === 'success' ? 'toast--success' : ''}`}>
      {toast.type === 'success' ? '✓ ' : ''}{toast.message}
    </div>
  );
}

// ========== Clipboard Helper ==========

function useCopy(showToast) {
  return useCallback((text, label = 'Copied') => {
    navigator.clipboard.writeText(text).then(() => showToast(label));
  }, [showToast]);
}

// ========== App ==========

function App() {
  const [tab, setTab] = useState('generate');
  const [serverStatus, setServerStatus] = useState(null);
  const { toast, showToast } = useToast();

  useEffect(() => {
    healthCheck()
      .then(() => setServerStatus('connected'))
      .catch(() => setServerStatus('disconnected'));
  }, []);

  return (
    <div className="app">
      <header className="header">
        <div className="header-left">
          <div className="logo">
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
              <rect x="1" y="1" width="6" height="6" rx="1" fill="white"/>
              <rect x="11" y="1" width="6" height="6" rx="1" fill="white"/>
              <rect x="1" y="11" width="6" height="6" rx="1" fill="white"/>
              <rect x="11" y="13" width="4" height="4" rx="1" fill="white" opacity="0.7"/>
              <rect x="13" y="11" width="4" height="4" rx="1" fill="white" opacity="0.5"/>
            </svg>
          </div>
          <h1 className="title">QR Service</h1>
          <span className="subtitle">Generate · Decode · Track</span>
        </div>
        <div className="header-right">
          {serverStatus && (
            <span
              className={`status-dot ${serverStatus === 'connected' ? 'status-dot--connected' : 'status-dot--disconnected'}`}
              title={serverStatus === 'connected' ? 'API connected' : 'API unreachable'}
            />
          )}
        </div>
      </header>

      <nav className="nav">
        {[
          ['generate', 'Generate'],
          ['decode', 'Decode'],
          ['templates', 'Templates'],
          ['tracked', 'Tracked'],
        ].map(([id, label]) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`nav__btn ${tab === id ? 'nav__btn--active' : ''}`}
          >
            {label}
          </button>
        ))}
      </nav>

      <main className="main">
        {tab === 'generate' && <GenerateTab showToast={showToast} />}
        {tab === 'decode' && <DecodeTab showToast={showToast} />}
        {tab === 'templates' && <TemplatesTab showToast={showToast} />}
        {tab === 'tracked' && <TrackedTab showToast={showToast} />}
      </main>

      <footer className="footer">
        <a href="/api/v1/openapi.json" target="_blank" rel="noopener">OpenAPI</a>
        <span className="footer__sep">·</span>
        <a href="/llms.txt" target="_blank" rel="noopener">llms.txt</a>
        <span className="footer__sep">·</span>
        <a href="/api/v1/health" target="_blank" rel="noopener">Health</a>
        <span className="footer__sep">·</span>
        <a href="https://github.com/Humans-Not-Required/qr-service" target="_blank" rel="noopener">GitHub</a>
      </footer>

      <Toast toast={toast} />
    </div>
  );
}

// ========== Generate Tab ==========

function GenerateTab({ showToast }) {
  const [data, setData] = useState('');
  const [format, setFormat] = useState('png');
  const [size, setSize] = useState(256);
  const [style, setStyle] = useState('square');
  const [fgColor, setFgColor] = useState('#000000');
  const [bgColor, setBgColor] = useState('#ffffff');
  const [ec, setEc] = useState('M');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const copy = useCopy(showToast);

  const handleGenerate = async (e) => {
    e.preventDefault();
    if (!data.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const { data: res } = await generateQR({
        data: data.trim(),
        format, size, style,
        fgColor: fgColor.replace('#', ''),
        bgColor: bgColor.replace('#', ''),
        errorCorrection: ec,
      });
      setResult(res);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <form onSubmit={handleGenerate} className="form">
        <div className="form-group">
          <label className="label">Content</label>
          <textarea
            value={data}
            onChange={e => setData(e.target.value)}
            placeholder="Enter a URL, text, or any data to encode as a QR code…"
            rows={3}
            className="textarea"
          />
        </div>

        <button
          type="button"
          onClick={() => setShowAdvanced(!showAdvanced)}
          className="advanced-toggle"
        >
          <span className={`advanced-toggle__icon ${showAdvanced ? 'advanced-toggle__icon--open' : ''}`}>▶</span>
          {showAdvanced ? 'Hide' : 'Show'} options
        </button>

        <div className={`advanced-section ${showAdvanced ? 'advanced-section--open' : 'advanced-section--closed'}`}>
          <div className="advanced-inner">
            <div className="form-row">
              <div className="form-group">
                <label className="label">Format</label>
                <select value={format} onChange={e => setFormat(e.target.value)} className="select">
                  {FORMATS.map(f => <option key={f} value={f}>{f.toUpperCase()}</option>)}
                </select>
              </div>
              <div className="form-group">
                <label className="label">Style</label>
                <select value={style} onChange={e => setStyle(e.target.value)} className="select">
                  {STYLES.map(s => <option key={s} value={s}>{s.charAt(0).toUpperCase() + s.slice(1)}</option>)}
                </select>
              </div>
              <div className="form-group">
                <label className="label">Size (px)</label>
                <input
                  type="number"
                  value={size}
                  onChange={e => setSize(+e.target.value)}
                  min={64}
                  max={4096}
                  className="input"
                />
              </div>
              <div className="form-group">
                <label className="label">Error Correction</label>
                <select value={ec} onChange={e => setEc(e.target.value)} className="select">
                  {EC_LEVELS.map(l => (
                    <option key={l} value={l}>
                      {l} ({l === 'L' ? '7%' : l === 'M' ? '15%' : l === 'Q' ? '25%' : '30%'})
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label className="label">Foreground</label>
                <div className="color-row">
                  <input type="color" value={fgColor} onChange={e => setFgColor(e.target.value)} className="color-picker" />
                  <input type="text" value={fgColor} onChange={e => setFgColor(e.target.value)} className="input" />
                </div>
              </div>
              <div className="form-group">
                <label className="label">Background</label>
                <div className="color-row">
                  <input type="color" value={bgColor} onChange={e => setBgColor(e.target.value)} className="color-picker" />
                  <input type="text" value={bgColor} onChange={e => setBgColor(e.target.value)} className="input" />
                </div>
              </div>
            </div>
          </div>
        </div>

        <button type="submit" disabled={loading || !data.trim()} className="btn btn--primary">
          {loading ? 'Generating…' : 'Generate QR Code'}
        </button>
      </form>

      {error && <div className="message--error">{error}</div>}

      {result && (
        <div className="result-card">
          <div className="card">
            <div className="qr-preview">
              {result.format === 'svg' ? (
                <div dangerouslySetInnerHTML={{ __html: atob(result.image_base64.replace('data:image/svg+xml;base64,', '')) }} />
              ) : (
                <img src={result.image_base64} alt="Generated QR code" className="qr-image" />
              )}
            </div>
            <div className="result-meta">
              <p><strong>Format:</strong> {result.format.toUpperCase()} · <strong>Size:</strong> {result.size}px</p>
              {result.share_url && (
                <p>
                  <strong>Share URL:</strong>{' '}
                  <a href={result.share_url} target="_blank" rel="noopener" className="link">
                    {result.share_url.length > 50 ? result.share_url.slice(0, 50) + '…' : result.share_url}
                  </a>
                  <button onClick={() => copy(result.share_url, 'Share URL copied')} className="btn btn--ghost btn--sm" title="Copy share URL">📋</button>
                </p>
              )}
              <div className="result-actions">
                <a href={result.image_base64} download={`qr.${result.format}`} className="btn btn--secondary">
                  ⬇ Download
                </a>
                <button onClick={() => copy(result.image_base64, 'Data URI copied')} className="btn btn--secondary">
                  📋 Copy Data URI
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ========== Decode Tab ==========

function DecodeTab({ showToast }) {
  const [imageData, setImageData] = useState('');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [dragOver, setDragOver] = useState(false);
  const copy = useCopy(showToast);

  const processFile = (file) => {
    const reader = new FileReader();
    reader.onload = () => setImageData(reader.result);
    reader.readAsDataURL(file);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file && file.type.startsWith('image/')) processFile(file);
  };

  const handleDecode = async () => {
    if (!imageData) return;
    setLoading(true);
    setError(null);
    try {
      const base64 = imageData.includes(',') ? imageData.split(',')[1] : imageData;
      const { data: res } = await decodeQR(base64);
      setResult(res);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <div
        onDragOver={e => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
        className={`drop-zone ${dragOver ? 'drop-zone--active' : ''}`}
      >
        {imageData ? (
          <img src={imageData} alt="QR to decode" style={{ maxWidth: 200, maxHeight: 200, borderRadius: 6 }} />
        ) : (
          <div>
            <div className="drop-zone__icon">📷</div>
            <p className="drop-zone__text">Drop a QR code image here</p>
            <label className="btn btn--secondary" style={{ cursor: 'pointer' }}>
              Browse Files
              <input
                type="file"
                accept="image/*"
                onChange={e => e.target.files[0] && processFile(e.target.files[0])}
                style={{ display: 'none' }}
              />
            </label>
          </div>
        )}
      </div>

      {imageData && (
        <div style={{ marginTop: '1rem', display: 'flex', gap: '0.5rem' }}>
          <button onClick={handleDecode} disabled={loading} className="btn btn--primary">
            {loading ? 'Decoding…' : 'Decode'}
          </button>
          <button onClick={() => { setImageData(''); setResult(null); setError(null); }} className="btn btn--secondary">
            Clear
          </button>
        </div>
      )}

      {error && <div className="message--error">{error}</div>}

      {result && (
        <div className="result-card">
          <div className="card" style={{ flexDirection: 'column' }}>
            <label className="label">Decoded Content</label>
            <pre className="pre">{result.data || result.content}</pre>
            <div>
              <button onClick={() => copy(result.data || result.content, 'Content copied')} className="btn btn--secondary">
                📋 Copy
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ========== Templates Tab ==========

function TemplatesTab({ showToast }) {
  const [template, setTemplate] = useState('url');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const copy = useCopy(showToast);

  const [url, setUrl] = useState('');
  const [ssid, setSsid] = useState('');
  const [password, setPassword] = useState('');
  const [encryption, setEncryption] = useState('WPA');
  const [firstName, setFirstName] = useState('');
  const [lastName, setLastName] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [org, setOrg] = useState('');

  const handleGenerate = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      let params;
      if (template === 'url') params = { url };
      else if (template === 'wifi') params = { ssid, password, encryption };
      else params = { first_name: firstName, last_name: lastName, email, phone, organization: org };
      const { data: res } = await generateFromTemplate(template, params);
      setResult(res);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <div className="sub-nav">
        {TEMPLATES.map(t => (
          <button
            key={t}
            onClick={() => { setTemplate(t); setResult(null); setError(null); }}
            className={`nav__btn ${template === t ? 'nav__btn--active' : ''}`}
          >
            {t === 'wifi' ? '📶 WiFi' : t === 'vcard' ? '👤 vCard' : '🔗 URL'}
          </button>
        ))}
      </div>

      <form onSubmit={handleGenerate} className="form">
        {template === 'url' && (
          <div className="form-group">
            <label className="label">URL</label>
            <input value={url} onChange={e => setUrl(e.target.value)} placeholder="https://example.com" className="input" />
          </div>
        )}

        {template === 'wifi' && (
          <>
            <div className="form-group">
              <label className="label">Network Name (SSID)</label>
              <input value={ssid} onChange={e => setSsid(e.target.value)} placeholder="MyWiFi" className="input" />
            </div>
            <div className="form-group">
              <label className="label">Password</label>
              <input type="password" value={password} onChange={e => setPassword(e.target.value)} placeholder="Password" className="input" />
            </div>
            <div className="form-group">
              <label className="label">Encryption</label>
              <select value={encryption} onChange={e => setEncryption(e.target.value)} className="select">
                <option value="WPA">WPA/WPA2</option>
                <option value="WEP">WEP</option>
                <option value="nopass">None</option>
              </select>
            </div>
          </>
        )}

        {template === 'vcard' && (
          <>
            <div className="form-row">
              <div className="form-group">
                <label className="label">First Name</label>
                <input value={firstName} onChange={e => setFirstName(e.target.value)} className="input" />
              </div>
              <div className="form-group">
                <label className="label">Last Name</label>
                <input value={lastName} onChange={e => setLastName(e.target.value)} className="input" />
              </div>
            </div>
            <div className="form-group">
              <label className="label">Email</label>
              <input type="email" value={email} onChange={e => setEmail(e.target.value)} className="input" />
            </div>
            <div className="form-group">
              <label className="label">Phone</label>
              <input value={phone} onChange={e => setPhone(e.target.value)} className="input" />
            </div>
            <div className="form-group">
              <label className="label">Organization</label>
              <input value={org} onChange={e => setOrg(e.target.value)} className="input" />
            </div>
          </>
        )}

        <button type="submit" disabled={loading} className="btn btn--primary">
          {loading ? 'Generating…' : 'Generate from Template'}
        </button>
      </form>

      {error && <div className="message--error">{error}</div>}

      {result && (
        <div className="result-card">
          <div className="card">
            <div className="qr-preview">
              <img src={result.image_base64 || result.image} alt="Template QR code" className="qr-image" />
            </div>
            <div className="result-meta">
              {result.share_url && (
                <p>
                  <strong>Share:</strong>{' '}
                  <a href={result.share_url} target="_blank" rel="noopener" className="link">View Link</a>
                  <button onClick={() => copy(result.share_url, 'Share URL copied')} className="btn btn--ghost btn--sm">📋</button>
                </p>
              )}
              <div className="result-actions">
                <a href={result.image_base64 || result.image} download={`qr-${template}.png`} className="btn btn--secondary">
                  ⬇ Download
                </a>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ========== Tracked Tab ==========

const TRACKED_STORAGE_KEY = 'qr-tracked-items';

function loadTrackedItems() {
  try { return JSON.parse(localStorage.getItem(TRACKED_STORAGE_KEY) || '[]'); }
  catch { return []; }
}

function saveTrackedItems(items) {
  localStorage.setItem(TRACKED_STORAGE_KEY, JSON.stringify(items));
}

function TrackedTab({ showToast }) {
  const [targetUrl, setTargetUrl] = useState('');
  const [shortCode, setShortCode] = useState('');
  const [expiresAt, setExpiresAt] = useState('');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [selectedId, setSelectedId] = useState(null);
  const [statsMap, setStatsMap] = useState({});
  const [dashLoading, setDashLoading] = useState(false);
  const [view, setView] = useState('dashboard');
  const [importToken, setImportToken] = useState('');
  const [importId, setImportId] = useState('');
  const [tracked, setTracked] = useState(() => loadTrackedItems());
  const copy = useCopy(showToast);

  useEffect(() => { saveTrackedItems(tracked); }, [tracked]);

  useEffect(() => {
    if (tracked.length === 0) return;
    setDashLoading(true);
    Promise.all(
      tracked.map(async (item) => {
        try {
          const { data: res } = await getTrackedStats(item.id, item.manage_token);
          return [item.id, res];
        } catch {
          return [item.id, { error: true, scan_count: item._lastScanCount || 0 }];
        }
      })
    ).then(results => {
      const map = {};
      results.forEach(([id, data]) => { map[id] = data; });
      setStatsMap(map);
      setDashLoading(false);
    });
  }, [tracked.length]);

  const refreshStats = async (item) => {
    try {
      const { data: res } = await getTrackedStats(item.id, item.manage_token);
      setStatsMap(prev => ({ ...prev, [item.id]: res }));
      setTracked(prev => prev.map(t => t.id === item.id ? { ...t, _lastScanCount: res.scan_count } : t));
      showToast('Stats refreshed');
    } catch (err) {
      setError(err.message);
    }
  };

  const handleCreate = async (e) => {
    e.preventDefault();
    if (!targetUrl.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const { data: res } = await createTrackedQR({
        targetUrl: targetUrl.trim(),
        shortCode: shortCode.trim() || undefined,
        expiresAt: expiresAt || undefined,
      });
      setResult(res);
      const newItem = {
        id: res.id,
        manage_token: res.manage_token,
        short_url: res.short_url,
        target_url: res.target_url,
        short_code: res.short_code,
        created_at: res.created_at || new Date().toISOString(),
        _lastScanCount: 0,
      };
      setTracked(prev => [newItem, ...prev]);
      setStatsMap(prev => ({ ...prev, [res.id]: { scan_count: 0, target_url: res.target_url, recent_scans: [] } }));
      setTargetUrl('');
      setShortCode('');
      setExpiresAt('');
      showToast('Tracked QR created');
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (item) => {
    if (!confirm(`Delete tracked QR "${item.short_code || item.id.slice(0, 8)}"?`)) return;
    try {
      await deleteTrackedQR(item.id, item.manage_token);
      setTracked(prev => prev.filter(t => t.id !== item.id));
      setStatsMap(prev => { const m = { ...prev }; delete m[item.id]; return m; });
      if (selectedId === item.id) setSelectedId(null);
      showToast('Tracked QR deleted');
    } catch (err) {
      setError(err.message);
    }
  };

  const handleImport = async (e) => {
    e.preventDefault();
    if (!importId.trim() || !importToken.trim()) return;
    setError(null);
    try {
      const { data: res } = await getTrackedStats(importId.trim(), importToken.trim());
      const newItem = {
        id: importId.trim(),
        manage_token: importToken.trim(),
        short_url: res.short_url || '',
        target_url: res.target_url,
        short_code: res.short_code || importId.trim().slice(0, 8),
        created_at: res.created_at || 'Unknown',
        _lastScanCount: res.scan_count,
      };
      setTracked(prev => {
        if (prev.some(t => t.id === newItem.id)) return prev;
        return [newItem, ...prev];
      });
      setStatsMap(prev => ({ ...prev, [importId.trim()]: res }));
      setImportId('');
      setImportToken('');
      showToast('Tracked QR imported');
    } catch (err) {
      setError('Import failed: ' + err.message);
    }
  };

  const totalScans = Object.values(statsMap).reduce((sum, s) => sum + (s.scan_count || 0), 0);
  const topPerformers = [...tracked]
    .map(t => ({ ...t, scans: statsMap[t.id]?.scan_count || 0 }))
    .sort((a, b) => b.scans - a.scans);
  const maxScans = topPerformers.length > 0 ? Math.max(topPerformers[0].scans, 1) : 1;

  const selectedStats = selectedId ? statsMap[selectedId] : null;
  const selectedItem = selectedId ? tracked.find(t => t.id === selectedId) : null;

  return (
    <div>
      <div className="sub-nav">
        {[['dashboard', '📊 Dashboard'], ['create', '+ Create'], ['import', '📥 Import']].map(([id, label]) => (
          <button
            key={id}
            onClick={() => { setView(id); setError(null); }}
            className={`nav__btn ${view === id ? 'nav__btn--active' : ''}`}
          >
            {label}
          </button>
        ))}
      </div>

      {error && <div className="message--error">{error}</div>}

      {/* ---- Create ---- */}
      {view === 'create' && (
        <div>
          <p className="hint" style={{ marginBottom: '1rem' }}>
            Create a tracked QR code with a short URL. Scans are logged and viewable in the dashboard.
          </p>
          <form onSubmit={handleCreate} className="form">
            <div className="form-group">
              <label className="label">Target URL</label>
              <input value={targetUrl} onChange={e => setTargetUrl(e.target.value)} placeholder="https://example.com" className="input" />
            </div>
            <div className="form-row">
              <div className="form-group">
                <label className="label">Custom Short Code <span className="hint">(optional)</span></label>
                <input value={shortCode} onChange={e => setShortCode(e.target.value)} placeholder="my-link" className="input" />
              </div>
              <div className="form-group">
                <label className="label">Expires At <span className="hint">(optional)</span></label>
                <input type="datetime-local" value={expiresAt} onChange={e => setExpiresAt(e.target.value)} className="input" />
              </div>
            </div>
            <button type="submit" disabled={loading || !targetUrl.trim()} className="btn btn--primary">
              {loading ? 'Creating…' : 'Create Tracked QR'}
            </button>
          </form>

          {result && (
            <div className="result-card">
              <div className="card">
                <div className="qr-preview">
                  <img src={result.qr?.image_base64 || result.image_base64} alt="Tracked QR code" className="qr-image" />
                </div>
                <div className="result-meta">
                  <p>
                    <strong>Short URL:</strong>{' '}
                    <code className="code">{result.short_url}</code>
                    <button onClick={() => copy(result.short_url, 'Short URL copied')} className="btn btn--ghost btn--sm">📋</button>
                  </p>
                  <p>
                    <strong>Manage Token:</strong>{' '}
                    <code className="code">{result.manage_token}</code>
                    <button onClick={() => copy(result.manage_token, 'Token copied')} className="btn btn--ghost btn--sm">📋</button>
                  </p>
                  <p className="warning-text">⚠️ Token saved to local storage. Copy it for backup.</p>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ---- Import ---- */}
      {view === 'import' && (
        <div>
          <p className="hint" style={{ marginBottom: '1rem' }}>
            Import an existing tracked QR code using its ID and manage token.
          </p>
          <form onSubmit={handleImport} className="form">
            <div className="form-group">
              <label className="label">Tracked QR ID</label>
              <input value={importId} onChange={e => setImportId(e.target.value)} placeholder="uuid…" className="input" />
            </div>
            <div className="form-group">
              <label className="label">Manage Token</label>
              <input value={importToken} onChange={e => setImportToken(e.target.value)} placeholder="Token from creation…" className="input" />
            </div>
            <button type="submit" disabled={!importId.trim() || !importToken.trim()} className="btn btn--primary">
              Import Tracked QR
            </button>
          </form>
        </div>
      )}

      {/* ---- Dashboard ---- */}
      {view === 'dashboard' && (
        <div>
          <div className="stat-grid">
            <div className="stat-card">
              <div className="stat-card__value">{tracked.length}</div>
              <div className="stat-card__label">Tracked QR Codes</div>
            </div>
            <div className="stat-card">
              <div className="stat-card__value">{totalScans}</div>
              <div className="stat-card__label">Total Scans</div>
            </div>
            <div className="stat-card">
              <div className="stat-card__value">{tracked.length > 0 ? (totalScans / tracked.length).toFixed(1) : '0'}</div>
              <div className="stat-card__label">Avg Scans / QR</div>
            </div>
          </div>

          {dashLoading && <p className="hint">Loading stats…</p>}

          {tracked.length === 0 ? (
            <div className="empty-state">
              <div className="empty-state__icon">📊</div>
              <p>No tracked QR codes yet</p>
              <p className="hint" style={{ marginTop: '0.25rem' }}>Create one or import an existing one to get started.</p>
            </div>
          ) : (
            <>
              <h3 className="section-title">All Tracked QR Codes</h3>
              {topPerformers.map(item => (
                <div
                  key={item.id}
                  onClick={() => setSelectedId(selectedId === item.id ? null : item.id)}
                  className={`qr-row ${selectedId === item.id ? 'qr-row--selected' : ''}`}
                >
                  <div className="qr-row__info">
                    <div className="qr-row__header">
                      <span className="qr-row__name">
                        {item.short_code || item.id.slice(0, 8)}
                      </span>
                      <span className="qr-row__target">→ {item.target_url}</span>
                    </div>
                    <div className="qr-row__bar-wrap">
                      <div className="bar-bg">
                        <div className="bar-fill" style={{ width: `${(item.scans / maxScans) * 100}%` }} />
                      </div>
                      <span className="qr-row__scans">
                        {item.scans} {item.scans === 1 ? 'scan' : 'scans'}
                      </span>
                    </div>
                  </div>
                  <div className="qr-row__actions">
                    <button
                      onClick={(e) => { e.stopPropagation(); refreshStats(item); }}
                      className="btn btn--ghost btn--sm"
                      title="Refresh stats"
                    >🔄</button>
                    <button
                      onClick={(e) => { e.stopPropagation(); handleDelete(item); }}
                      className="btn btn--ghost btn--sm btn--danger"
                      title="Delete"
                    >🗑️</button>
                  </div>
                </div>
              ))}

              {selectedId && selectedStats && selectedItem && (
                <div className="detail-panel">
                  <div className="card">
                    <div className="detail-header">
                      <h3>📈 {selectedItem.short_code || selectedId.slice(0, 8)} — Details</h3>
                      <button onClick={() => setSelectedId(null)} className="btn btn--ghost btn--sm">✕</button>
                    </div>
                    <div>
                      <p className="detail-row">
                        <strong>Target:</strong>{' '}
                        <a href={selectedStats.target_url} target="_blank" rel="noopener" className="link">{selectedStats.target_url}</a>
                      </p>
                      <p className="detail-row"><strong>Total Scans:</strong> {selectedStats.scan_count}</p>
                      {selectedStats.short_code && (
                        <p className="detail-row"><strong>Short Code:</strong> <code className="code">{selectedStats.short_code}</code></p>
                      )}
                      {selectedStats.expires_at && (
                        <p className="detail-row"><strong>Expires:</strong> {new Date(selectedStats.expires_at).toLocaleString()}</p>
                      )}
                      {selectedItem.created_at && (
                        <p className="detail-row"><strong>Created:</strong> {new Date(selectedItem.created_at).toLocaleString()}</p>
                      )}
                    </div>

                    {selectedStats.recent_scans?.length > 0 ? (
                      <div>
                        <h4 className="label" style={{ marginTop: '0.5rem', marginBottom: '0.4rem' }}>Recent Scans</h4>
                        <div className="scans-list">
                          {selectedStats.recent_scans.map((s, i) => (
                            <div key={i} className="scan-row">
                              <span style={{ color: 'var(--text-primary)', fontSize: '0.8rem' }}>{new Date(s.scanned_at).toLocaleString()}</span>
                              <span style={{ color: 'var(--text-muted)', fontSize: '0.75rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                                {s.user_agent?.slice(0, 60) || 'Unknown agent'}
                              </span>
                              {s.referrer && <span style={{ color: 'var(--accent)', fontSize: '0.75rem' }}>via {s.referrer}</span>}
                            </div>
                          ))}
                        </div>
                      </div>
                    ) : (
                      <p className="hint">No scans recorded yet.</p>
                    )}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}

export default App
