import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('便携版运行时', () => {
  it('release EXE 使用 Windows 图形子系统，不创建额外控制台窗口', async () => {
    const main = await readFile(
      resolve(process.cwd(), 'src-tauri/src/main.rs'),
      'utf8',
    )

    expect(main).toContain('#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]')
  })
})
