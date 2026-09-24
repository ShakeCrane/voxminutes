'use client'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { SettingsSection } from './SettingsSection'
import {
  apiGetTranscriptConfig, customLocalDelete, customLocalList,
  customLocalSelect, customLocalTest, customLocalUpsert, getTranslationEngine,
  type CustomLocalProfile,
} from '@/services/ipc'

const fresh = (): CustomLocalProfile => ({
  id: '', name: '', task: 'asr',
  endpoint: 'http://127.0.0.1:8000/v1', model: '', timeoutSecs: 120,
})

/** Model files belong to the user's server; this UI registers its inference API. */
export function CustomLocalModelsSection() {
  const [profiles, setProfiles] = useState<CustomLocalProfile[]>([])
  const [form, setForm] = useState<CustomLocalProfile>(fresh)
  const [asr, setAsr] = useState('')
  const [translation, setTranslation] = useState('')
  const [busy, setBusy] = useState(false)

  const refresh = async () => {
    const [items, transcript, engine] = await Promise.all([
      customLocalList(), apiGetTranscriptConfig(), getTranslationEngine(),
    ])
    setProfiles(items)
    setAsr(transcript?.provider === 'custom-local' ? transcript.model : '')
    setTranslation(engine.startsWith('custom:') ? engine.slice(7) : '')
  }
  useEffect(() => {
    refresh().catch((e) => toast.error(String(e)))
  }, [])

  const save = async () => {
    setBusy(true)
    try {
      await customLocalUpsert(form)
      setForm(fresh())
      await refresh()
      toast.success('已保存本地模型配置')
    } catch (e) {
      toast.error(String(e))
    } finally {
      setBusy(false)
    }
  }
  const test = async () => {
    setBusy(true)
    try { toast.success(await customLocalTest(form)) }
    catch (e) { toast.error(String(e)) }
    finally { setBusy(false) }
  }
  const select = async (item: CustomLocalProfile) => {
    try {
      await customLocalSelect(item.id)
      await refresh()
      toast.success('已启用；下次转写/翻译将使用此模型')
    } catch (e) { toast.error(String(e)) }
  }
  const remove = async (item: CustomLocalProfile) => {
    if (!window.confirm('删除配置 ' + item.name + '？模型文件不会被删除。')) return
    try {
      await customLocalDelete(item.id)
      if (form.id === item.id) setForm(fresh())
      await refresh()
      toast.success('已删除配置')
    } catch (e) { toast.error(String(e)) }
  }

  return (
    <SettingsSection title="自定义本地模型 / Custom local models">
      <p className="text-xs text-muted-foreground mb-3">
        连接你自己运行的 OpenAI 兼容推理服务（例如 Whisper / Qwen3-ASR、
        OpenVINO、llama.cpp 或 Ollama 的兼容接口）。模型文件及设备设置由服务器负责；
        VoxMinutes 不会自动转换模型或保证 NPU 支持。只允许本机 localhost 地址。
      </p>
      <div className="flex flex-col gap-2">
        <label className="text-xs">显示名称
          <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })}
            placeholder="e.g. Whisper on Intel NPU" />
        </label>
        <label className="text-xs">任务
          <select className="w-full rounded-md border bg-background p-2 mt-1"
            value={form.task} disabled={!!form.id}
            onChange={(e) => setForm({ ...form, task: e.target.value as CustomLocalProfile['task'] })}>
            <option value="asr">语音识别 / ASR</option>
            <option value="translation">翻译 / Translation</option>
            <option value="summary">总结 / Summary</option>
          </select>
        </label>
        <label className="text-xs">模型 ID（服务器 /models 返回的名称）
          <Input value={form.model} onChange={(e) => setForm({ ...form, model: e.target.value })}
            placeholder="model-id" />
        </label>
        <label className="text-xs">本地 API 根地址（以 /v1 结束，或服务根目录）
          <Input value={form.endpoint} onChange={(e) => setForm({ ...form, endpoint: e.target.value })}
            placeholder="http://127.0.0.1:8000/v1" />
        </label>
        <label className="text-xs">请求超时（秒）
          <Input type="number" min={1} max={3600} value={form.timeoutSecs}
            onChange={(e) => setForm({ ...form, timeoutSecs: Number(e.target.value) })} />
        </label>
        <div className="flex gap-2 flex-wrap">
          <Button disabled={busy || !form.name.trim() || !form.model.trim()} onClick={save}>
            {form.id ? '保存修改' : '添加模型'}
          </Button>
          <Button variant="outline" disabled={busy} onClick={test}>测试连接</Button>
          <Button variant="outline" onClick={() => setForm(fresh())}>清空</Button>
        </div>
      </div>
      <p className="text-xs text-muted-foreground mt-4">
        ASR 需要 POST /audio/transcriptions（WAV multipart）；翻译和总结需要
        POST /chat/completions。总结模型在会议总结的“本地模型”列表中选择；
        录音中途切换 ASR 不影响已开始的会话。请先启动推理服务。
      </p>
      <div className="mt-3 flex flex-col gap-2">
        {profiles.map((item) => {
          const active = (item.task === 'asr' && item.id === asr)
            || (item.task === 'translation' && item.id === translation)
          return (
            <div key={item.id} className="rounded-md border px-3 py-2 flex items-center gap-2 flex-wrap">
              <div className="flex-1 min-w-0">
                <div className="font-medium text-sm truncate">{item.name} {active ? '✓' : ''}</div>
                <div className="text-xs text-muted-foreground break-all">
                  {item.task} · {item.model} · {item.endpoint}
                </div>
              </div>
              {item.task !== 'summary' && (
                <Button size="sm" variant={active ? 'outline' : 'default'}
                  disabled={active || busy} onClick={() => select(item)}>
                  {active ? '已启用' : '启用'}
                </Button>
              )}
              <Button size="sm" variant="outline" onClick={() => setForm(item)}>编辑</Button>
              <Button size="sm" variant="outline" disabled={active || busy}
                onClick={() => remove(item)}>删除</Button>
            </div>
          )
        })}
      </div>
    </SettingsSection>
  )
}
