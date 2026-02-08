import { useState, useCallback } from 'react'
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

function TrackedTab({ onRateLimit }) {
  const [targetUrl, setTargetUrl] = useState('');
  const [shortCode, setShortCode] = useState('');
  const [expiresAt, setExpiresAt] = useState('');
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [stats, setStats] = useState(null);
  const [statsLoading, setStatsLoading] = useState(false);

  // Store created tracked QRs with their manage tokens in session
  const [created, setCreated] = useState([]);

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
      setCreated(prev => [{ ...res }, ...prev]);
      onRateLimit(rateLimit);
    } catch (err) {
      setError(err.message);
      if (err.rateLimit) onRateLimit(err.rateLimit);
    } finally {
      setLoading(false);
    }
  };

  const loadStats = async (item) => {
    setStatsLoading(true);
    setStats(null);
    try {
      const { data: res } = await getTrackedStats(item.id, item.manage_token);
      setStats({ ...res, id: item.id });
    } catch (err) {
      setError(err.message);
    } finally {
      setStatsLoading(false);
    }
  };

  const handleDelete = async (item) => {
    if (!confirm('Delete this tracked QR code?')) return;
    try {
      await deleteTrackedQR(item.id, item.manage_token);
      setCreated(prev => prev.filter(c => c.id !== item.id));
      if (stats?.id === item.id) setStats(null);
    } catch (err) {
      setError(err.message);
    }
  };

  return (
    <div>
      <p style={styles.hint}>Create QR codes with short URLs that track scan analytics. Each tracked QR returns a manage token for viewing stats and deleting.</p>

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

      {error && <div style={styles.error}>{error}</div>}

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
            <p style={{ ...styles.hint, color: '#fbbf24' }}>⚠️ Save your manage token — it's shown only once!</p>
          </div>
        </div>
      )}

      {created.length > 0 && (
        <div style={{ marginTop: '2rem' }}>
          <h3 style={{ color: '#f8fafc', fontSize: '1rem', marginBottom: '0.75rem' }}>Created This Session</h3>
          {created.map(item => (
            <div key={item.id} style={{ ...styles.resultCard, marginTop: '0.5rem', padding: '0.75rem' }}>
              <div style={{ flex: 1 }}>
                <p style={{ margin: 0, fontSize: '0.85rem' }}>
                  <strong>{item.short_url}</strong> → {item.target_url}
                </p>
                <p style={styles.hint}>Scans: {item.scan_count || 0}</p>
              </div>
              <div style={{ display: 'flex', gap: '0.25rem' }}>
                <button onClick={() => loadStats(item)} style={styles.secondaryBtn} disabled={statsLoading}>📊</button>
                <button onClick={() => handleDelete(item)} style={{ ...styles.secondaryBtn, color: '#ef4444' }}>🗑️</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {stats && (
        <div style={{ ...styles.resultCard, marginTop: '1rem' }}>
          <div>
            <h3 style={{ color: '#f8fafc', fontSize: '1rem', margin: '0 0 0.5rem' }}>Scan Stats</h3>
            <p><strong>Total scans:</strong> {stats.scan_count}</p>
            <p><strong>Target:</strong> {stats.target_url}</p>
            {stats.expires_at && <p><strong>Expires:</strong> {new Date(stats.expires_at).toLocaleString()}</p>}
            {stats.recent_scans?.length > 0 && (
              <div style={{ marginTop: '0.5rem' }}>
                <strong>Recent scans:</strong>
                {stats.recent_scans.slice(0, 5).map((s, i) => (
                  <p key={i} style={styles.hint}>{new Date(s.scanned_at).toLocaleString()} — {s.user_agent?.slice(0, 50) || 'Unknown'}</p>
                ))}
              </div>
            )}
          </div>
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
