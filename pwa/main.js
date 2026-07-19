// SatsPath PWA — main.js
// El .js del WASM está en src/wasm/ (Vite lo procesa como módulo).
// El .wasm binario está en public/ (se sirve estáticamente con MIME type correcto).

import init, {
  generate_identity_keypair,
  sign_profile_json,
  verify_signed_profile,
} from './src/wasm/satspath_wasm.js';

let keypair = null;
let selectedRail = 'lightning';

// ── Init ─────────────────────────────────────────────────────────────────────

async function run() {
  const statusEl = document.getElementById('wasm-status');
  const dotEl    = document.getElementById('wasm-dot');

  try {
    await init('/satspath_wasm_bg.wasm');

    statusEl.textContent  = '✓ WASM activo';
    statusEl.style.color  = 'var(--green)';
    dotEl.style.background = 'var(--green)';
    dotEl.style.animation  = 'none';

    document.getElementById('btn-gen').disabled     = false;
    document.getElementById('btn-receive').disabled = false;

    console.log('[SatsPath] WASM inicializado ✓');
  } catch (e) {
    statusEl.textContent = `✗ Error WASM: ${e.message}`;
    statusEl.style.color = 'var(--red)';
    dotEl.style.background = 'var(--red)';
    console.error('[SatsPath] WASM init falló:', e);
    return;
  }

  // ── Generate Keypair ──────────────────────────────────────────────────────

  document.getElementById('btn-gen').addEventListener('click', () => {
    try {
      keypair = generate_identity_keypair();

      const outEl = document.getElementById('out-keys');
      outEl.textContent =
        `Pubkey (hex):\n${keypair.pubkey_hex}\n\nPrivate key: [HIDDEN — en memoria, no en disco]`;
      outEl.style.display = 'block';

      // Habilitar botón de firma
      document.getElementById('btn-sign').disabled = false;

      fireLaser('laser-identity');
      console.log('[SatsPath] Keypair generado ✓', keypair.pubkey_hex.slice(0, 16) + '…');
    } catch (e) {
      console.error(e);
      document.getElementById('out-keys').textContent = `Error: ${e}`;
      document.getElementById('out-keys').style.display = 'block';
    }
  });

  // ── Sign Profile ──────────────────────────────────────────────────────────

  document.getElementById('btn-sign').addEventListener('click', () => {
    if (!keypair) {
      alert('Primero genera un keypair.');
      return;
    }
    try {
      const profile = {
        alias: 'demo@satspath.dev',
        identity_pubkey: keypair.pubkey_hex,
        methods: [
          { Lightning: { lightning_address: 'demo@getalby.com', label: 'Lightning' } },
        ],
        preferences: ['lightning'],
      };

      const sigHex = sign_profile_json(JSON.stringify(profile), keypair.secret_key_hex);
      const signedProfile = { profile, signature: sigHex };
      const signedJson = JSON.stringify(signedProfile, null, 2);
      const isValid = verify_signed_profile(signedJson);

      const outEl = document.getElementById('out-sig');
      outEl.textContent = isValid
        ? `✓ FIRMA VÁLIDA\n\nSignature:\n${sigHex}\n\nPerfil completo:\n${signedJson}`
        : `✗ FIRMA INVÁLIDA\n\n${sigHex}`;
      outEl.className = `output ${isValid ? 'valid' : 'invalid'}`;
      outEl.style.display = 'block';

      fireLaser('laser-sign');
    } catch (e) {
      console.error(e);
      document.getElementById('out-sig').textContent = `Error: ${e}`;
      document.getElementById('out-sig').style.display = 'block';
    }
  });

  // ── Receive ───────────────────────────────────────────────────────────────

  document.getElementById('btn-receive').addEventListener('click', () => {
    const amountSats = parseInt(document.getElementById('amount-input').value) || 21000;
    const rail = window._selectedRail || 'lightning';

    let uri = '';
    let label = '';

    if (rail === 'lightning') {
      // Genera un placeholder de Lightning Address (en producción va al resolver)
      const addr = keypair
        ? `${keypair.pubkey_hex.slice(0, 8)}@satspath.dev`
        : 'tu@satspath.dev';
      uri   = `lightning:${addr}?amount=${amountSats}`;
      label = `⚡ Lightning Address\n${addr}`;
    } else if (rail === 'ark') {
      const vtxo = keypair
        ? `ark1${keypair.pubkey_hex.slice(0, 20)}`
        : 'ark1xxxxxx';
      uri   = `ark:${vtxo}?amount=${amountSats}`;
      label = `🌳 Ark VTXO\n${vtxo}`;
    } else {
      // On-chain BIP-21
      const addr = keypair
        ? `bc1p${keypair.pubkey_hex.slice(2, 40)}`
        : 'bc1pxxxxxxxx';
      const btcAmount = (amountSats / 100_000_000).toFixed(8);
      uri   = `bitcoin:${addr}?amount=${btcAmount}`;
      label = `₿ On-chain\n${addr}`;
    }

    const outEl = document.getElementById('out-receive');
    outEl.textContent = `${label}\n\nURI de pago:\n${uri}\n\nMonto: ${amountSats.toLocaleString()} sats`;
    outEl.style.display = 'block';

    // Update QR placeholder
    document.getElementById('qr-box').textContent = uri.slice(0, 40) + '…';

    fireLaser('laser-receive');
  });

  // ── Rail sync ─────────────────────────────────────────────────────────────
  // Sincronizar el rail seleccionado desde el HTML global
  const railBtns = ['rail-lightning', 'rail-ark', 'rail-onchain'];
  railBtns.forEach(id => {
    document.getElementById(id)?.addEventListener('click', () => {
      window._selectedRail = id.replace('rail-', '');
    });
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function fireLaser(id) {
  const el = document.getElementById(id);
  if (!el) return;
  el.classList.remove('fire');
  void el.offsetWidth;
  el.classList.add('fire');
  el.addEventListener('animationend', () => el.classList.remove('fire'), { once: true });
}

run();
