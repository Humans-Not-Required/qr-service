const API_BASE = import.meta.env.VITE_API_URL || '/api/v1';

async function request(path, options = {}) {
  const apiKey = localStorage.getItem('qr_api_key') || '';
  const headers = {
    'Content-Type': 'application/json',
    ...(apiKey && { 'Authorization': `Bearer ${apiKey}` }),
    ...options.headers,
  };

  const res = await fetch(`${API_BASE}${path}`, { ...options, headers });

  // Extract rate limit headers
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

export async function generateQR({ data, format = 'png', size = 256, style = 'square', fgColor = '000000', bgColor = 'ffffff', errorCorrection = 'M' }) {
  return request('/qr/generate', {
    method: 'POST',
    body: JSON.stringify({
      data,
      format,
      size,
      style,
      fg_color: fgColor,
      bg_color: bgColor,
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

export async function getHistory(page = 1, perPage = 20) {
  return request(`/qr/history?page=${page}&per_page=${perPage}`);
}

export async function getQRById(id) {
  return request(`/qr/${id}`);
}

export async function deleteQR(id) {
  return request(`/qr/${id}`, { method: 'DELETE' });
}

export async function getImageUrl(id) {
  const apiKey = localStorage.getItem('qr_api_key') || '';
  return `${API_BASE}/qr/${id}/image${apiKey ? `?key=${encodeURIComponent(apiKey)}` : ''}`;
}

export async function createTrackedQR({ targetUrl, shortCode, expiresAt, ...qrOptions }) {
  return request('/qr/tracked', {
    method: 'POST',
    body: JSON.stringify({
      target_url: targetUrl,
      short_code: shortCode || undefined,
      expires_at: expiresAt || undefined,
      ...qrOptions,
    }),
  });
}

export async function getTrackedQRs(page = 1, perPage = 20) {
  return request(`/qr/tracked?page=${page}&per_page=${perPage}`);
}

export async function getTrackedStats(id) {
  return request(`/qr/tracked/${id}/stats`);
}

export async function deleteTrackedQR(id) {
  return request(`/qr/tracked/${id}`, { method: 'DELETE' });
}

export async function healthCheck() {
  return request('/health');
}
