import { useState, useEffect, useCallback } from 'react'
import { generateQR, decodeQR, getHistory, deleteQR, generateFromTemplate, healthCheck } from './api'

const STYLES = ['square', 'rounded', 'dots'];
const FORMATS = ['png', 'svg'];
const EC_LEVELS = ['L', 'M', 'Q', 'H'];
const TEMPLATES = ['wifi', 'vcard', 'url'];

function App() {
  const [tab, setTab] = useState('generate');
  const [apiKey, setApiKey] = useState(() => localStorage.getItem('qr_api_key') || '');
  const [showSettings, setShowSettings] = useState(!apiKey);
  const [serverStatus, setServerStatus] = useState(null);
  const [rateLimit, setRateLimit] = useState(null);

  useEffect(() => {
    localStorage.setItem('qr_api_key', apiKey);
  }, [apiKey]);

  const checkHealth = useCallback(async () => {
    try {
      await healthCheck();
      setServerStatus('connected');
    } catch {
      setServerStatus('disconnected');
    }
  }, []);

  useEffect(() => {
    if (apiKey) checkHealth();
  }, [apiKey, checkHealth]);

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
          <button onClick={() => setShowSettings(s => !s)} style={styles.settingsBtn}>⚙️</button>
        </div>
      </header>

      {showSettings && (
        <div style={styles.settingsPanel}>
          <label style={styles.label}>API Key</label>
          <input
            type="password"
            value={apiKey}
            onChange={e => setApiKey(e.target.value)}
            placeholder="Enter your API key..."
            style={styles.input}
          />
          <p style={styles.hint}>Your key is stored locally and never sent to third parties.</p>
        </div>
      )}

      <nav style={styles.nav}>
        {[
          ['generate', '🔳 Generate'],
          ['decode', '🔍 Decode'],
          ['templates', '📋 Templates'],
          ['history', '📜 History'],
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
        {tab === 'history' && <HistoryTab onRateLimit={setRateLimit} />}
      </main>

      <footer style={styles.footer}>
        <a href="/api/v1/openapi.json" target="_blank" rel="noopener" style={styles.footerLink}>OpenAPI Spec</a>
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
              <div dangerouslySetInnerHTML={{ __html: atob(result.image.replace('data:image/svg+xml;base64,', '')) }} />
            ) : (
              <img src={result.image} alt="Generated QR code" style={styles.qrImage} />
            )}
          </div>
          <div style={styles.resultMeta}>
            <p><strong>ID:</strong> <code style={styles.code}>{result.id}</code></p>
            <p><strong>Format:</strong> {result.format.toUpperCase()}</p>
            <p><strong>Size:</strong> {result.size}px</p>
            <p><strong>Style:</strong> {result.style}</p>
            <div style={styles.resultActions}>
              <a href={result.image} download={`qr-${result.id}.${result.format}`} style={styles.secondaryBtn}>⬇️ Download</a>
              <button onClick={() => navigator.clipboard.writeText(result.image)} style={styles.secondaryBtn}>📋 Copy Data URI</button>
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
    reader.onload = () => {
      setImageData(reader.result);
    };
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

  // URL template
  const [url, setUrl] = useState('');

  // WiFi template
  const [ssid, setSsid] = useState('');
  const [password, setPassword] = useState('');
  const [encryption, setEncryption] = useState('WPA');

  // vCard template
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
      if (template === 'url') {
        params = { url };
      } else if (template === 'wifi') {
        params = { ssid, password, encryption };
      } else {
        params = { first_name: firstName, last_name: lastName, email, phone, organization: org };
      }
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
            <img src={result.image} alt="Template QR code" style={styles.qrImage} />
          </div>
          <div style={styles.resultMeta}>
            <p><strong>ID:</strong> <code style={styles.code}>{result.id}</code></p>
            <a href={result.image} download={`qr-${template}-${result.id}.png`} style={styles.secondaryBtn}>⬇️ Download</a>
          </div>
        </div>
      )}
    </div>
  );
}

function HistoryTab({ onRateLimit }) {
  const [codes, setCodes] = useState([]);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  const loadHistory = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const { data: res, rateLimit } = await getHistory(page, 12);
      setCodes(res.items || res.codes || []);
      setTotal(res.total || 0);
      onRateLimit(rateLimit);
    } catch (err) {
      setError(err.message);
      if (err.rateLimit) onRateLimit(err.rateLimit);
    } finally {
      setLoading(false);
    }
  }, [page, onRateLimit]);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  const handleDelete = async (id) => {
    if (!confirm('Delete this QR code?')) return;
    try {
      await deleteQR(id);
      loadHistory();
    } catch (err) {
      setError(err.message);
    }
  };

  const totalPages = Math.ceil(total / 12);

  return (
    <div>
      {loading && <p style={styles.hint}>Loading...</p>}
      {error && <div style={styles.error}>{error}</div>}

      {codes.length === 0 && !loading && (
        <div style={styles.emptyState}>
          <p style={{ fontSize: '2rem' }}>📭</p>
          <p>No QR codes yet. Generate one to get started!</p>
        </div>
      )}

      <div style={styles.historyGrid}>
        {codes.map(code => (
          <div key={code.id} style={styles.historyCard}>
            <img src={code.image} alt="QR code" style={styles.historyImage} />
            <div style={styles.historyMeta}>
              <p style={styles.historyData} title={code.data}>
                {code.data?.length > 40 ? code.data.slice(0, 40) + '…' : code.data}
              </p>
              <p style={styles.historyDate}>{new Date(code.created_at).toLocaleString()}</p>
              <div style={{ display: 'flex', gap: '0.25rem' }}>
                <span style={styles.tag}>{code.format}</span>
                {code.style && code.style !== 'square' && <span style={styles.tag}>{code.style}</span>}
              </div>
            </div>
            <button onClick={() => handleDelete(code.id)} style={styles.deleteBtn} title="Delete">🗑️</button>
          </div>
        ))}
      </div>

      {totalPages > 1 && (
        <div style={styles.pagination}>
          <button onClick={() => setPage(p => Math.max(1, p - 1))} disabled={page <= 1} style={styles.secondaryBtn}>← Prev</button>
          <span style={styles.hint}>Page {page} of {totalPages}</span>
          <button onClick={() => setPage(p => Math.min(totalPages, p + 1))} disabled={page >= totalPages} style={styles.secondaryBtn}>Next →</button>
        </div>
      )}
    </div>
  );
}

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
  settingsBtn: { background: 'none', border: 'none', fontSize: '1.25rem', cursor: 'pointer' },
  settingsPanel: {
    backgroundColor: '#1e293b',
    borderRadius: 8,
    padding: '1rem',
    marginBottom: '1rem',
  },
  nav: {
    display: 'flex',
    gap: '0.25rem',
    marginBottom: '1.5rem',
    borderBottom: '1px solid #1e293b',
    paddingBottom: '0.5rem',
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
  historyGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(250px, 1fr))',
    gap: '1rem',
  },
  historyCard: {
    backgroundColor: '#1e293b',
    borderRadius: 8,
    padding: '1rem',
    display: 'flex',
    gap: '0.75rem',
    alignItems: 'center',
    position: 'relative',
  },
  historyImage: { width: 64, height: 64, borderRadius: 4, imageRendering: 'pixelated', flexShrink: 0 },
  historyMeta: { flex: 1, minWidth: 0 },
  historyData: { margin: 0, fontSize: '0.85rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
  historyDate: { margin: '0.25rem 0 0', fontSize: '0.75rem', color: '#64748b' },
  tag: {
    fontSize: '0.7rem',
    backgroundColor: '#374151',
    borderRadius: 3,
    padding: '0.1rem 0.35rem',
    color: '#94a3b8',
    textTransform: 'uppercase',
  },
  deleteBtn: {
    position: 'absolute',
    top: 8,
    right: 8,
    background: 'none',
    border: 'none',
    cursor: 'pointer',
    fontSize: '0.85rem',
    opacity: 0.5,
    transition: 'opacity 0.15s',
  },
  pagination: {
    display: 'flex',
    justifyContent: 'center',
    alignItems: 'center',
    gap: '1rem',
    marginTop: '1.5rem',
  },
  emptyState: { textAlign: 'center', padding: '3rem 1rem', color: '#64748b' },
};

export default App
