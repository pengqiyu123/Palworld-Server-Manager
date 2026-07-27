import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

// QA 收官（严过关）前端单元测试配置。
// 仅用于 `npx vitest run`，不干扰原 vite build 流程。
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['tests/**/*.spec.ts'],
    maxWorkers: 1,
    fileParallelism: false,
  },
})
