import path from 'path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import eslint from 'vite-plugin-eslint'
import basicSsl from '@vitejs/plugin-basic-ssl'
import tailwindcss from '@tailwindcss/vite'
import basicAuth from './plugins/vite-plugin-basic-auth'

// https://vitejs.dev/config/
export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  plugins: [
    basicAuth(),
    react(),
    tailwindcss(),
    eslint({
      include: ['src/**/*.ts', 'src/**/*.tsx'],
      exclude: ['src/test/**/*.ts', 'src/test/**/*.tsx'],
      cache: false,
    }),
    process.env.HTTPS === 'true' && basicSsl(),
  ].filter(Boolean),
  server: {
    port: 3002,
    host: true,
    allowedHosts: ['.trycloudflare.com'],
    fs: {
      allow: [
        '..',
        '../crates/satspath-wasm/pkg'
      ]
    }
  },
  build: {
    emptyOutDir: true,
    sourcemap: true,
    rollupOptions: {
      external: ['fs'],
    },
  },
  optimizeDeps: {
    exclude: ['satspath-wasm']
  },
  worker: {
    format: 'es',
  },
})
