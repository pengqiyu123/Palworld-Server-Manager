import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5222,
    strictPort: true,
    watch: {
      ignored: [
        resolve(__dirname, 'src-tauri'),
        resolve(__dirname, 'target'),
        '**/src-tauri/**',
        '**/target/**',
      ]
    }
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src')
    }
  }
})
