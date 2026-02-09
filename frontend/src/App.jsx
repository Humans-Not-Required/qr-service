import { useState, useCallback, useEffect, useRef } from 'react'
import { generateQR, decodeQR, generateFromTemplate, createTrackedQR, getTrackedStats, deleteTrackedQR, healthCheck } from './api'

const STYLES = ['square', 'rounded', 'dots'];
const FORMATS = ['png', 'svg'];
const EC_LEVELS = ['L', 'M', 'Q', 'H'];
const TEMPLATES = ['wifi', 'vcard', 'url'];

function App() {
  const [tab, setTab] = useState('generate');
  const [serverStatus, setServerStatus] = useState(null);
  const [rateLimit, setRateLimit] = useState(null);

  useState(() => {
    healthCheck().then(() => setServerStatus('connected')).catch(() => setServerStatus('disconnected'));
  });

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <div style={styles.headerLeft}>
          <h1 style={styles.title}>⬛ QR Service</h1>
          <span style={styles.subtitle}>Agent-First QR Code API</span>
        </div>
        <div style={styles.headerRight}>
          {serverStatus && (
            <span style={{
              ...styles.statusDot,
              backgroundColor: serverStatus === 'connected' ? '#10b981' : '#ef4444'
            }} title={serverStatus} />
          )}
          {rateLimit?.remaining != null && (
            <span style={styles.rateBadge}>
              {rateLimit.remaining}/{rateLimit.limit} req
            </span>
          )}
        </div>
      </header>

      <nav style={styles.nav}>
        {[
          ['generate', '🔳 Generate'],
          ['decode', '🔍 Decode'],
          ['templates', '📋 Templates'],
          ['tracked', '📊 Tracked'],
        ].map(([id, label]) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            style={tab === id ? { ...styles.navBtn, ...styles.navBtnActive } : styles.navBtn}
          >
            {label}
          </button>
        ))}
      </nav>

      <main style={styles.main}>
        {tab === 'generate' && <GenerateTab onRateLimit={setRateLimit} />}
        {tab === 'decode' && <DecodeTab onRateLimit={setRateLimit} />}
        {tab === 'templates' && <TemplatesTab onRateLimit={setRateLimit} />}
        {tab === 'tracked' && <TrackedTab onRateLimit={setRateLimit} />}
      </main>

      <footer style={styles.footer}>
        <a href="/api/v1/openapi.json" target="_blank" rel="noopener" style={styles.footerLink}>OpenAPI Spec</a>
        <span style={styles.footerSep}>·</span>
        <a href="/api/v1/health" target="_blank" rel="noopener" style={styles.footerLink}>Health</a>
        <span style={styles.footerSep}>·</span>
        <a href="https://github.com/Humans-Not-Required/qr-service" target="_blank" rel="noopener" style={styles.footerLink}>GitHub</a>
        <span style={styles.footerSep}>·</span>
        <span style={styles.footerText}>Humans Not Required</span>
      </footer>
    </div>
  );
}

function GenerateTab({ onRateLimit }) {
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

  const handleGenerate = async (e) => {
    e.preventDefault();
    if (!data.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const { data: res, rateLimit } = await generateQR({
        data: data.trim(),
        format,
        size,
        style,
        fgColor: fgColor.replace('#', ''),
        bgColor: bgColor.replace('#', ''),
        errorCorrection: ec,
      });
      setResult(res);
      onRateLimit(rateLimit);
    } catch (err) {
      setError(err.message);
      if (err.rateLimit) onRateLimit(err.rateLimit);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <form onSubmit={handleGenerate} style={styles.form}>
        <div style={styles.formGroup}>
          <label style={styles.label}>Content *</label>
          <textarea
            value={data}
            onChange={e => setData(e.target.value)}
            placeholder="URL, text, or any data to encode..."
            rows={3}
            style={styles.textarea}
          />
        </div>

        <div style={styles.formRow}>
          <div style={styles.formGroup}>
            <label style={styles.label}>Format</label>
            <select value={format} onChange={e => setFormat(e.target.value)} style={styles.select}>
              {FORMATS.map(f => <option key={f} value={f}>{f.toUpperCase()}</option>)}
            </select>
          </div>
          <div style={styles.formGroup}>
            <label style={styles.label}>Style</label>
            <select value={style} onChange={e => setStyle(e.target.value)} style={styles.select}>
              {STYLES.map(s => <option key={s} value={s}>{s.charAt(0).toUpperCase() + s.slice(1)}</option>)}
            </select>
          </div>
          <div style={styles.formGroup}>
            <label style={styles.label}>Size (px)</label>
            <input type="number" value={size} onChange={e => setSize(+e.target.value)} min={64} max={4096} style={styles.input} />
          </div>
          <div style={styles.formGroup}>
            <label style={styles.label}>Error Correction</label>
            <select value={ec} onChange={e => setEc(e.target.value)} style={styles.select}>
              {EC_LEVELS.map(l => <option key={l} value={l}>{l} ({l === 'L' ? '7%' : l === 'M' ? '15%' : l === 'Q' ? '25%' : '30%'})</option>)}
            </select>
          </div>
        </div>

        <div style={styles.formRow}>
          <div style={styles.formGroup}>
            <label style={styles.label}>Foreground</label>
            <div style={styles.colorRow}>
              <input type="color" value={fgColor} onChange={e => setFgColor(e.target.value)} style={styles.colorPicker} />
              <input type="text" value={fgColor} onChange={e => setFgColor(e.target.value)} style={{ ...styles.input, flex: 1 }} />
            </div>
          </div>
          <div style={styles.formGroup}>
            <label style={styles.label}>Background</label>
            <div style={styles.colorRow}>
              <input type="color" value={bgColor} onChange={e => setBgColor(e.target.value)} style={styles.colorPicker} />
              <input type="text" value={bgColor} onChange={e => setBgColor(e.target.value)} style={{ ...styles.input, flex: 1 }} />
            </div>
          </div>
        </div>

        <button type="submit" disabled={loading || !data.trim()} style={styles.primaryBtn}>
          {loading ? 'Generating...' : '🔳 Generate QR Code'}
        </button>
      </form>

      {error && <div style={styles.error}>{error}</div>}

      {result && (
        <div style={styles.resultCard}>
          <div style={styles.qrPreview}>
            {result.format === 'svg' ? (
              <div dangerouslySetInnerHTML={{ __html: atob(result.image_base64.replace('data:image/svg+xml;base64,', '')) }} />
            ) : (
              <img src={result.image_base64} alt="Generated QR code" style={styles.qrImage} />
            )}
          </div>
          <div style={styles.resultMeta}>
            <p><strong>Format:</strong> {result.format.toUpperCase()} · <strong>Size:</strong> {result.size}px</p>
            {result.share_url && (
              <p>
                <strong>Share:</strong>{' '}
                <a href={result.share_url} target="_blank" rel="noopener" style={styles.footerLink}>
                  {result.share_url.length > 60 ? result.share_url.slice(0, 60) + '…' : result.share_url}
                </a>
                <button onClick={() => navigator.clipboard.writeText(result.share_url)} style={{ ...styles.secondaryBtn, marginLeft: '0.5rem', padding: '0.2rem 0.5rem', fontSize: '0.75rem' }}>📋</button>
              </p>
            )}
            <div style={styles.resultActions}>
              <a href={result.image_base64} download={`qr.${result.format}`} style={styles.secondaryBtn}>⬇️ Download</a>
              <button onClick={() => navigator.clipboard.writeText(result.image_base64)} style={styles.secondaryBtn}>📋 Copy Data URI</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function DecodeTab({ onRateLimit }) {
  const [imageData, setImageData] = useState('');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [dragOver, setDragOver] = useState(false);

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
      const { data: res, rateLimit } = await decodeQR(base64);
      setResult(res);
      onRateLimit(rateLimit);
    } catch (err) {
      setError(err.message);
      if (err.rateLimit) onRateLimit(err.rateLimit);
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
        style={{
          ...styles.dropZone,
          borderColor: dragOver ? '#6366f1' : '#374151',
          backgroundColor: dragOver ? 'rgba(99,102,241,0.05)' : 'transparent',
        }}
      >
        {imageData ? (
          <img src={imageData} alt="QR to decode" style={{ maxWidth: 200, maxHeight: 200 }} />
        ) : (
          <div>
            <p style={{ fontSize: '2rem', margin: 0 }}>📷</p>
            <p>Drop a QR code image here or</p>
            <label style={styles.secondaryBtn}>
              Browse Files
              <input type="file" accept="image/*" onChange={e => e.target.files[0] && processFile(e.target.files[0])} style={{ display: 'none' }} />
            </label>
          </div>
        )}
      </div>

      {imageData && (
        <div style={{ marginTop: '1rem', display: 'flex', gap: '0.5rem' }}>
          <button onClick={handleDecode} disabled={loading} style={styles.primaryBtn}>
            {loading ? 'Decoding...' : '🔍 Decode'}
          </button>
          <button onClick={() => { setImageData(''); setResult(null); }} style={styles.secondaryBtn}>Clear</button>
        </div>
      )}

      {error && <div style={styles.error}>{error}</div>}

      {result && (
        <div style={styles.resultCard}>
          <label style={styles.label}>Decoded Content</label>
          <pre style={styles.pre}>{result.data || result.content}</pre>
          <button onClick={() => navigator.clipboard.writeText(result.data || result.content)} style={styles.secondaryBtn}>📋 Copy</button>
        </div>
      )}
    </div>
  );
}

function TemplatesTab({ onRateLimit }) {
  const [template, setTemplate] = useState('url');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

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
      const { data: res, rateLimit } = await generateFromTemplate(template, params);
      setResult(res);
      onRateLimit(rateLimit);
    } catch (err) {
      setError(err.message);
      if (err.rateLimit) onRateLimit(err.rateLimit);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <div style={styles.formRow}>
        {TEMPLATES.map(t => (
          <button
            key={t}
            onClick={() => { setTemplate(t); setResult(null); }}
            style={template === t ? { ...styles.navBtn, ...styles.navBtnActive } : styles.navBtn}
          >
            {t === 'wifi' ? '📶 WiFi' : t === 'vcard' ? '👤 vCard' : '🔗 URL'}
          </button>
        ))}
      </div>

      <form onSubmit={handleGenerate} style={styles.form}>
        {template === 'url' && (
          <div style={styles.formGroup}>
            <label style={styles.label}>URL</label>
            <input value={url} onChange={e => setUrl(e.target.value)} placeholder="https://example.com" style={styles.input} />
          </div>
        )}

        {template === 'wifi' && (
          <>
            <div style={styles.formGroup}>
              <label style={styles.label}>Network Name (SSID)</label>
              <input value={ssid} onChange={e => setSsid(e.target.value)} placeholder="MyWiFi" style={styles.input} />
            </div>
            <div style={styles.formGroup}>
              <label style={styles.label}>Password</label>
              <input type="password" value={password} onChange={e => setPassword(e.target.value)} placeholder="Password" style={styles.input} />
            </div>
            <div style={styles.formGroup}>
              <label style={styles.label}>Encryption</label>
              <select value={encryption} onChange={e => setEncryption(e.target.value)} style={styles.select}>
                <option value="WPA">WPA/WPA2</option>
                <option value="WEP">WEP</option>
                <option value="nopass">None</option>
              </select>
            </div>
          </>
        )}

        {template === 'vcard' && (
          <>
            <div style={styles.formRow}>
              <div style={styles.formGroup}>
                <label style={styles.label}>First Name</label>
                <input value={firstName} onChange={e => setFirstName(e.target.value)} style={styles.input} />
              </div>
              <div style={styles.formGroup}>
                <label style={styles.label}>Last Name</label>
                <input value={lastName} onChange={e => setLastName(e.target.value)} style={styles.input} />
              </div>
            </div>
            <div style={styles.formGroup}>
              <label style={styles.label}>Email</label>
              <input type="email" value={email} onChange={e => setEmail(e.target.value)} style={styles.input} />
            </div>
            <div style={styles.formGroup}>
              <label style={styles.label}>Phone</label>
              <input value={phone} onChange={e => setPhone(e.target.value)} style={styles.input} />
            </div>
            <div style={styles.formGroup}>
              <label style={styles.label}>Organization</label>
              <input value={org} onChange={e => setOrg(e.target.value)} style={styles.input} />
            </div>
          </>
        )}

        <button type="submit" disabled={loading} style={styles.primaryBtn}>
          {loading ? 'Generating...' : '🔳 Generate from Template'}
        </button>
      </form>

      {error && <div style={styles.error}>{error}</div>}

      {result && (
        <div style={styles.resultCard}>
          <div style={styles.qrPreview}>
            <img src={result.image_base64 || result.image} alt="Template QR code" style={styles.qrImage} />
          </div>
          <div style={styles.resultMeta}>
            {result.share_url && (
              <p>
                <strong>Share:</strong>{' '}
                <a href={result.share_url} target="_blank" rel="noopener" style={styles.footerLink}>View Link</a>
                <button onClick={() => navigator.clipboard.writeText(result.share_url)} style={{ ...styles.secondaryBtn, marginLeft: '0.5rem', padding: '0.2rem 0.5rem', fontSize: '0.75rem' }}>📋</button>
              </p>
            )}
            <a href={result.image_base64 || result.image} download={`qr-${template}.png`} style={styles.secondaryBtn}>⬇️ Download</a>
          </div>
        </div>
      )}
    </div>
  );
}

// localStorage helpers for tracked QR persistence
const TRACKED_STORAGE_KEY = 'qr-tracked-items';

function loadTrackedItems() {
  try {
    return JSON.parse(localStorage.getItem(TRACKED_STORAGE_KEY) || '[]');
  } catch { return []; }
}

function saveTrackedItems(items) {
  localStorage.setItem(TRACKED_STORAGE_KEY, JSON.stringify(items));
}

function TrackedTab({ onRateLimit }) {
  const [targetUrl, setTargetUrl] = useState('');
  const [shortCode, setShortCode] = useState('');
  const [expiresAt, setExpiresAt] = useState('');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [selectedId, setSelectedId] = useState(null);
  const [statsMap, setStatsMap] = useState({});
  const [dashLoading, setDashLoading] = useState(false);
  const [view, setView] = useState('dashboard'); // 'dashboard' | 'create'
  const [importToken, setImportToken] = useState('');
  const [importId, setImportId] = useState('');

  // Persistent tracked items
  const [tracked, setTracked] = useState(() => loadTrackedItems());

  // Sync to localStorage
  useEffect(() => { saveTrackedItems(tracked); }, [tracked]);

  // Load stats for all tracked items on mount
  useEffect(() => {
    if (tracked.length === 0) return;
    setDashLoading(true);
    const promises = tracked.map(async (item) => {
      try {
        const { data: res } = await getTrackedStats(item.id, item.manage_token);
        return [item.id, res];
      } catch {
        return [item.id, { error: true, scan_count: item._lastScanCount || 0 }];
      }
    });
    Promise.all(promises).then(results => {
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
      // Cache last known scan count
      setTracked(prev => prev.map(t => t.id === item.id ? { ...t, _lastScanCount: res.scan_count } : t));
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
      const { data: res, rateLimit } = await createTrackedQR({
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
      onRateLimit(rateLimit);
      setTargetUrl('');
      setShortCode('');
      setExpiresAt('');
    } catch (err) {
      setError(err.message);
      if (err.rateLimit) onRateLimit(err.rateLimit);
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
    } catch (err) {
      setError('Import failed: ' + err.message);
    }
  };

  // Dashboard summary
  const totalScans = Object.values(statsMap).reduce((sum, s) => sum + (s.scan_count || 0), 0);
  const activeCount = tracked.length;
  const topPerformers = [...tracked]
    .map(t => ({ ...t, scans: statsMap[t.id]?.scan_count || 0 }))
    .sort((a, b) => b.scans - a.scans);
  const maxScans = topPerformers.length > 0 ? Math.max(topPerformers[0].scans, 1) : 1;

  const selectedStats = selectedId ? statsMap[selectedId] : null;
  const selectedItem = selectedId ? tracked.find(t => t.id === selectedId) : null;

  return (
    <div>
      {/* View toggle */}
      <div style={{ display: 'flex', gap: '0.5rem', marginBottom: '1rem' }}>
        <button
          onClick={() => setView('dashboard')}
          style={view === 'dashboard' ? { ...styles.navBtn, ...styles.navBtnActive } : styles.navBtn}
        >📊 Dashboard</button>
        <button
          onClick={() => setView('create')}
          style={view === 'create' ? { ...styles.navBtn, ...styles.navBtnActive } : styles.navBtn}
        >➕ Create</button>
        <button
          onClick={() => setView('import')}
          style={view === 'import' ? { ...styles.navBtn, ...styles.navBtnActive } : styles.navBtn}
        >📥 Import</button>
      </div>

      {error && <div style={styles.error}>{error}</div>}

      {/* Create view */}
      {view === 'create' && (
        <div>
          <p style={styles.hint}>Create a tracked QR code with a short URL. Scans are logged and viewable in the dashboard.</p>
          <form onSubmit={handleCreate} style={{ ...styles.form, marginTop: '1rem' }}>
            <div style={styles.formGroup}>
              <label style={styles.label}>Target URL *</label>
              <input value={targetUrl} onChange={e => setTargetUrl(e.target.value)} placeholder="https://example.com" style={styles.input} />
            </div>
            <div style={styles.formRow}>
              <div style={styles.formGroup}>
                <label style={styles.label}>Custom Short Code (optional)</label>
                <input value={shortCode} onChange={e => setShortCode(e.target.value)} placeholder="my-link" style={styles.input} />
              </div>
              <div style={styles.formGroup}>
                <label style={styles.label}>Expires At (optional)</label>
                <input type="datetime-local" value={expiresAt} onChange={e => setExpiresAt(e.target.value)} style={styles.input} />
              </div>
            </div>
            <button type="submit" disabled={loading || !targetUrl.trim()} style={styles.primaryBtn}>
              {loading ? 'Creating...' : '📊 Create Tracked QR'}
            </button>
          </form>

          {result && (
            <div style={styles.resultCard}>
              <div style={styles.qrPreview}>
                <img src={result.qr?.image_base64 || result.image_base64} alt="Tracked QR code" style={styles.qrImage} />
              </div>
              <div style={styles.resultMeta}>
                <p><strong>Short URL:</strong> <code style={styles.code}>{result.short_url}</code>
                  <button onClick={() => navigator.clipboard.writeText(result.short_url)} style={{ ...styles.secondaryBtn, marginLeft: '0.5rem', padding: '0.2rem 0.5rem', fontSize: '0.75rem' }}>📋</button>
                </p>
                <p><strong>Manage Token:</strong> <code style={styles.code}>{result.manage_token}</code>
                  <button onClick={() => navigator.clipboard.writeText(result.manage_token)} style={{ ...styles.secondaryBtn, marginLeft: '0.5rem', padding: '0.2rem 0.5rem', fontSize: '0.75rem' }}>📋</button>
                </p>
                <p style={{ ...styles.hint, color: '#fbbf24' }}>⚠️ Token saved to local storage. You can also copy it for backup.</p>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Import view */}
      {view === 'import' && (
        <div>
          <p style={styles.hint}>Import an existing tracked QR code using its ID and manage token. This adds it to your local dashboard.</p>
          <form onSubmit={handleImport} style={{ ...styles.form, marginTop: '1rem' }}>
            <div style={styles.formGroup}>
              <label style={styles.label}>Tracked QR ID *</label>
              <input value={importId} onChange={e => setImportId(e.target.value)} placeholder="uuid..." style={styles.input} />
            </div>
            <div style={styles.formGroup}>
              <label style={styles.label}>Manage Token *</label>
              <input value={importToken} onChange={e => setImportToken(e.target.value)} placeholder="Token from creation..." style={styles.input} />
            </div>
            <button type="submit" disabled={!importId.trim() || !importToken.trim()} style={styles.primaryBtn}>
              📥 Import Tracked QR
            </button>
          </form>
        </div>
      )}

      {/* Dashboard view */}
      {view === 'dashboard' && (
        <div>
          {/* Stats summary */}
          <div style={{ display: 'flex', gap: '1rem', marginBottom: '1.5rem', flexWrap: 'wrap' }}>
            <div style={dashStyles.statCard}>
              <div style={dashStyles.statValue}>{activeCount}</div>
              <div style={dashStyles.statLabel}>Tracked QR Codes</div>
            </div>
            <div style={dashStyles.statCard}>
              <div style={dashStyles.statValue}>{totalScans}</div>
              <div style={dashStyles.statLabel}>Total Scans</div>
            </div>
            <div style={dashStyles.statCard}>
              <div style={dashStyles.statValue}>{activeCount > 0 ? (totalScans / activeCount).toFixed(1) : '0'}</div>
              <div style={dashStyles.statLabel}>Avg Scans/QR</div>
            </div>
          </div>

          {dashLoading && <p style={styles.hint}>Loading stats...</p>}

          {tracked.length === 0 ? (
            <div style={styles.emptyState}>
              <p style={{ fontSize: '2rem', margin: '0 0 0.5rem' }}>📊</p>
              <p>No tracked QR codes yet.</p>
              <p style={styles.hint}>Create one in the Create tab, or import an existing one.</p>
            </div>
          ) : (
            <>
              {/* QR list with inline bar chart */}
              <h3 style={{ color: '#f8fafc', fontSize: '1rem', marginBottom: '0.75rem' }}>All Tracked QR Codes</h3>
              {topPerformers.map(item => (
                <div
                  key={item.id}
                  onClick={() => setSelectedId(selectedId === item.id ? null : item.id)}
                  style={{
                    ...dashStyles.qrRow,
                    borderColor: selectedId === item.id ? '#6366f1' : '#1e293b',
                    cursor: 'pointer',
                  }}
                >
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                      <span style={{ fontWeight: 600, color: '#f8fafc', fontSize: '0.9rem' }}>
                        {item.short_code || item.id.slice(0, 8)}
                      </span>
                      <span style={{ color: '#64748b', fontSize: '0.8rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        → {item.target_url}
                      </span>
                    </div>
                    {/* Bar chart */}
                    <div style={{ marginTop: '0.4rem', display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                      <div style={dashStyles.barBg}>
                        <div style={{ ...dashStyles.barFill, width: `${(item.scans / maxScans) * 100}%` }} />
                      </div>
                      <span style={{ fontSize: '0.8rem', color: '#94a3b8', minWidth: '40px', textAlign: 'right' }}>
                        {item.scans} {item.scans === 1 ? 'scan' : 'scans'}
                      </span>
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: '0.25rem', alignItems: 'center' }}>
                    <button
                      onClick={(e) => { e.stopPropagation(); refreshStats(item); }}
                      style={{ ...styles.secondaryBtn, padding: '0.3rem 0.5rem', fontSize: '0.75rem' }}
                      title="Refresh stats"
                    >🔄</button>
                    <button
                      onClick={(e) => { e.stopPropagation(); handleDelete(item); }}
                      style={{ ...styles.secondaryBtn, padding: '0.3rem 0.5rem', fontSize: '0.75rem', color: '#ef4444' }}
                      title="Delete"
                    >🗑️</button>
                  </div>
                </div>
              ))}

              {/* Detail panel for selected QR */}
              {selectedId && selectedStats && selectedItem && (
                <div style={{ ...styles.resultCard, marginTop: '1rem', flexDirection: 'column' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <h3 style={{ color: '#f8fafc', fontSize: '1rem', margin: 0 }}>
                      📈 {selectedItem.short_code || selectedId.slice(0, 8)} — Details
                    </h3>
                    <button onClick={() => setSelectedId(null)} style={{ ...styles.secondaryBtn, padding: '0.2rem 0.5rem', fontSize: '0.75rem' }}>✕</button>
                  </div>
                  <div style={{ marginTop: '0.75rem' }}>
                    <p style={{ margin: '0.25rem 0', fontSize: '0.9rem' }}><strong>Target:</strong> <a href={selectedStats.target_url} target="_blank" rel="noopener" style={styles.footerLink}>{selectedStats.target_url}</a></p>
                    <p style={{ margin: '0.25rem 0', fontSize: '0.9rem' }}><strong>Total Scans:</strong> {selectedStats.scan_count}</p>
                    {selectedStats.short_code && <p style={{ margin: '0.25rem 0', fontSize: '0.9rem' }}><strong>Short Code:</strong> <code style={styles.code}>{selectedStats.short_code}</code></p>}
                    {selectedStats.expires_at && <p style={{ margin: '0.25rem 0', fontSize: '0.9rem' }}><strong>Expires:</strong> {new Date(selectedStats.expires_at).toLocaleString()}</p>}
                    {selectedItem.created_at && <p style={{ margin: '0.25rem 0', fontSize: '0.9rem' }}><strong>Created:</strong> {new Date(selectedItem.created_at).toLocaleString()}</p>}
                  </div>

                  {selectedStats.recent_scans?.length > 0 && (
                    <div style={{ marginTop: '1rem' }}>
                      <h4 style={{ color: '#94a3b8', fontSize: '0.85rem', margin: '0 0 0.5rem' }}>Recent Scans</h4>
                      <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
                        {selectedStats.recent_scans.map((s, i) => (
                          <div key={i} style={dashStyles.scanRow}>
                            <span style={{ color: '#e5e7eb', fontSize: '0.8rem' }}>{new Date(s.scanned_at).toLocaleString()}</span>
                            <span style={{ color: '#64748b', fontSize: '0.75rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '300px' }}>
                              {s.user_agent?.slice(0, 60) || 'Unknown agent'}
                            </span>
                            {s.referrer && <span style={{ color: '#6366f1', fontSize: '0.75rem' }}>via {s.referrer}</span>}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {(!selectedStats.recent_scans || selectedStats.recent_scans.length === 0) && (
                    <p style={{ ...styles.hint, marginTop: '1rem' }}>No scans recorded yet.</p>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}

const dashStyles = {
  statCard: {
    flex: '1 1 120px',
    backgroundColor: '#1e293b',
    borderRadius: 8,
    padding: '1rem',
    textAlign: 'center',
    border: '1px solid #334155',
  },
  statValue: {
    fontSize: '1.75rem',
    fontWeight: 700,
    color: '#f8fafc',
    lineHeight: 1,
  },
  statLabel: {
    fontSize: '0.75rem',
    color: '#64748b',
    marginTop: '0.25rem',
    textTransform: 'uppercase',
    letterSpacing: '0.05em',
  },
  qrRow: {
    display: 'flex',
    alignItems: 'center',
    gap: '0.75rem',
    padding: '0.75rem 1rem',
    backgroundColor: '#1e293b',
    borderRadius: 8,
    marginBottom: '0.5rem',
    border: '1px solid #1e293b',
    transition: 'border-color 0.15s',
  },
  barBg: {
    flex: 1,
    height: 6,
    backgroundColor: '#0f172a',
    borderRadius: 3,
    overflow: 'hidden',
  },
  barFill: {
    height: '100%',
    backgroundColor: '#6366f1',
    borderRadius: 3,
    transition: 'width 0.3s ease',
    minWidth: 2,
  },
  scanRow: {
    display: 'flex',
    flexDirection: 'column',
    gap: '0.15rem',
    padding: '0.5rem 0',
    borderBottom: '1px solid #0f172a',
  },
};

// ========== Styles ==========

const styles = {
  container: {
    maxWidth: 960,
    margin: '0 auto',
    padding: '1rem',
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
    color: '#e5e7eb',
    backgroundColor: '#0f172a',
    minHeight: '100vh',
  },
  header: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: '1rem 0',
    borderBottom: '1px solid #1e293b',
    marginBottom: '1rem',
  },
  headerLeft: { display: 'flex', alignItems: 'baseline', gap: '0.75rem' },
  headerRight: { display: 'flex', alignItems: 'center', gap: '0.75rem' },
  title: { margin: 0, fontSize: '1.5rem', color: '#f8fafc' },
  subtitle: { fontSize: '0.85rem', color: '#64748b' },
  statusDot: { width: 8, height: 8, borderRadius: '50%', display: 'inline-block' },
  rateBadge: {
    fontSize: '0.75rem',
    color: '#94a3b8',
    backgroundColor: '#1e293b',
    padding: '0.2rem 0.5rem',
    borderRadius: 4,
  },
  nav: {
    display: 'flex',
    gap: '0.25rem',
    marginBottom: '1.5rem',
    borderBottom: '1px solid #1e293b',
    paddingBottom: '0.5rem',
    flexWrap: 'wrap',
  },
  navBtn: {
    background: 'none',
    border: 'none',
    color: '#94a3b8',
    padding: '0.5rem 1rem',
    cursor: 'pointer',
    borderRadius: '6px 6px 0 0',
    fontSize: '0.9rem',
    transition: 'all 0.15s',
  },
  navBtnActive: {
    color: '#f8fafc',
    backgroundColor: '#1e293b',
  },
  main: { minHeight: 400 },
  footer: {
    marginTop: '2rem',
    padding: '1rem 0',
    borderTop: '1px solid #1e293b',
    textAlign: 'center',
    fontSize: '0.8rem',
    color: '#64748b',
  },
  footerLink: { color: '#6366f1', textDecoration: 'none' },
  footerSep: { margin: '0 0.5rem' },
  footerText: {},
  form: { display: 'flex', flexDirection: 'column', gap: '1rem' },
  formGroup: { display: 'flex', flexDirection: 'column', gap: '0.25rem', flex: 1 },
  formRow: { display: 'flex', gap: '1rem', flexWrap: 'wrap' },
  label: { fontSize: '0.85rem', color: '#94a3b8', fontWeight: 500 },
  input: {
    backgroundColor: '#1e293b',
    border: '1px solid #374151',
    borderRadius: 6,
    padding: '0.5rem 0.75rem',
    color: '#e5e7eb',
    fontSize: '0.9rem',
    outline: 'none',
  },
  textarea: {
    backgroundColor: '#1e293b',
    border: '1px solid #374151',
    borderRadius: 6,
    padding: '0.5rem 0.75rem',
    color: '#e5e7eb',
    fontSize: '0.9rem',
    resize: 'vertical',
    fontFamily: 'inherit',
    outline: 'none',
  },
  select: {
    backgroundColor: '#1e293b',
    border: '1px solid #374151',
    borderRadius: 6,
    padding: '0.5rem 0.75rem',
    color: '#e5e7eb',
    fontSize: '0.9rem',
    outline: 'none',
  },
  colorRow: { display: 'flex', gap: '0.5rem', alignItems: 'center' },
  colorPicker: { width: 36, height: 36, border: 'none', borderRadius: 4, cursor: 'pointer', padding: 0 },
  primaryBtn: {
    backgroundColor: '#6366f1',
    color: '#fff',
    border: 'none',
    borderRadius: 6,
    padding: '0.6rem 1.25rem',
    fontSize: '0.9rem',
    cursor: 'pointer',
    fontWeight: 500,
    transition: 'opacity 0.15s',
  },
  secondaryBtn: {
    backgroundColor: '#1e293b',
    color: '#e5e7eb',
    border: '1px solid #374151',
    borderRadius: 6,
    padding: '0.5rem 1rem',
    fontSize: '0.85rem',
    cursor: 'pointer',
    textDecoration: 'none',
    display: 'inline-block',
  },
  error: {
    backgroundColor: 'rgba(239,68,68,0.1)',
    border: '1px solid #ef4444',
    borderRadius: 6,
    padding: '0.75rem',
    color: '#fca5a5',
    marginTop: '1rem',
    fontSize: '0.9rem',
  },
  hint: { fontSize: '0.8rem', color: '#64748b', margin: '0.25rem 0 0' },
  resultCard: {
    display: 'flex',
    gap: '1.5rem',
    marginTop: '1.5rem',
    backgroundColor: '#1e293b',
    borderRadius: 8,
    padding: '1.5rem',
    flexWrap: 'wrap',
  },
  qrPreview: { display: 'flex', alignItems: 'center', justifyContent: 'center' },
  qrImage: { maxWidth: 256, maxHeight: 256, borderRadius: 4, imageRendering: 'pixelated' },
  resultMeta: { display: 'flex', flexDirection: 'column', gap: '0.5rem', flex: 1, justifyContent: 'center' },
  resultActions: { display: 'flex', gap: '0.5rem', marginTop: '0.5rem', flexWrap: 'wrap' },
  code: { backgroundColor: '#0f172a', padding: '0.15rem 0.4rem', borderRadius: 4, fontSize: '0.8rem' },
  pre: {
    backgroundColor: '#0f172a',
    borderRadius: 6,
    padding: '1rem',
    overflowX: 'auto',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-all',
    fontSize: '0.9rem',
  },
  dropZone: {
    border: '2px dashed #374151',
    borderRadius: 8,
    padding: '2rem',
    textAlign: 'center',
    cursor: 'pointer',
    transition: 'all 0.2s',
  },
  emptyState: { textAlign: 'center', padding: '3rem 1rem', color: '#64748b' },
};

export default App;
