const API_BASE = import.meta.env.VITE_API_URL || '/api/v1';

async function request(path, options = {}) {
  const headers = {
    'Content-Type': 'application/json',
    ...options.headers,
  };

  const res = await fetch(`${API_BASE}${path}`, { ...options, headers });

  const rateLimit = {
    limit: res.headers.get('X-RateLimit-Limit'),
    remaining: res.headers.get('X-RateLimit-Remaining'),
    reset: res.headers.get('X-RateLimit-Reset'),
  };

  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    const err = new Error(body.error || `HTTP ${res.status}`);
    err.status = res.status;
    err.code = body.code;
    err.rateLimit = rateLimit;
    throw err;
  }

  const data = await res.json();
  return { data, rateLimit };
}

function authRequest(path, manageToken, options = {}) {
  const headers = manageToken ? { 'Authorization': `Bearer ${manageToken}` } : {};
  return request(path, { ...options, headers: { ...headers, ...options.headers } });
}

export async function generateQR({ data, format = 'png', size = 256, style = 'square', fgColor = '000000', bgColor = 'ffffff', errorCorrection = 'M' }) {
  return request('/qr/generate', {
    method: 'POST',
    body: JSON.stringify({
      data, format, size, style,
      fg_color: fgColor, bg_color: bgColor,
      error_correction: errorCorrection,
    }),
  });
}

export async function decodeQR(imageBase64) {
  return request('/qr/decode', {
    method: 'POST',
    body: JSON.stringify({ image: imageBase64 }),
  });
}

export async function batchGenerate(items) {
  return request('/qr/batch', {
    method: 'POST',
    body: JSON.stringify({ items }),
  });
}

export async function generateFromTemplate(type, params) {
  return request(`/qr/template/${type}`, {
    method: 'POST',
    body: JSON.stringify(params),
  });
}

export async function createTrackedQR({ targetUrl, shortCode, expiresAt, ...qrOptions }, manageToken) {
  return authRequest('/qr/tracked', manageToken, {
    method: 'POST',
    body: JSON.stringify({
      target_url: targetUrl,
      short_code: shortCode || undefined,
      expires_at: expiresAt || undefined,
      ...qrOptions,
    }),
  });
}

export async function getTrackedStats(id, manageToken) {
  return authRequest(`/qr/tracked/${id}/stats`, manageToken);
}

export async function deleteTrackedQR(id, manageToken) {
  return authRequest(`/qr/tracked/${id}`, manageToken, { method: 'DELETE' });
}

export async function healthCheck() {
  return request('/health');
}
