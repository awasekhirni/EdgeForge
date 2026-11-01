import { useState, useEffect } from 'react'
import ChatPanel from './components/ChatPanel.jsx'
import ModelPanel from './components/ModelPanel.jsx'
import HealthBar from './components/HealthBar.jsx'
import MonitorPanel from './components/MonitorPanel.jsx'

const DEFAULT_API_URL = 'http://127.0.0.1:11434'

/**
 * Top-level EdgeForge desktop app.
 *
 * Tabs:
 *  - Chat: chat completion UI
 *  - Models: list + import models
 *  - Monitor: SSD health, thermal, throughput
 *
 * The HealthBar at the bottom always shows the server's health status.
 */
export default function App() {
  const [apiUrl, setApiUrl] = useState(DEFAULT_API_URL)
  const [apiKey, setApiKey] = useState('')
  const [tab, setTab] = useState('chat')
  const [health, setHealth] = useState(null)
  const [healthError, setHealthError] = useState(null)

  // Poll /health every 5 seconds.
  useEffect(() => {
    let cancelled = false

    async function checkHealth() {
      try {
        const headers = {}
        if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`
        const resp = await fetch(`${apiUrl}/health`, { headers })
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
        const body = await resp.json()
        if (!cancelled) {
          setHealth(body)
          setHealthError(null)
        }
      } catch (e) {
        if (!cancelled) {
          setHealth(null)
          setHealthError(e.message)
        }
      }
    }

    checkHealth()
    const interval = setInterval(checkHealth, 5000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [apiUrl, apiKey])

  return (
    <div className={`bg-app min-h-screen flex flex-col ${tab === 'light' ? 'theme-light' : ''}`}>
      {/* Header */}
      <header className="border-b border-slate-700 px-6 py-3 flex items-center gap-4">
        <h1 className="text-xl font-bold text-cyan-400">EdgeForge</h1>
        <nav className="flex gap-1">
          <button
            onClick={() => setTab('chat')}
            className={`px-3 py-1 rounded ${tab === 'chat' ? 'bg-cyan-600 text-white' : 'text-slate-400 hover:text-white'}`}
          >
            Chat
          </button>
          <button
            onClick={() => setTab('models')}
            className={`px-3 py-1 rounded ${tab === 'models' ? 'bg-cyan-600 text-white' : 'text-slate-400 hover:text-white'}`}
          >
            Models
          </button>
          <button
            onClick={() => setTab('monitor')}
            className={`px-3 py-1 rounded ${tab === 'monitor' ? 'bg-cyan-600 text-white' : 'text-slate-400 hover:text-white'}`}
          >
            Monitor
          </button>
        </nav>
        <div className="flex-1" />
        <input
          type="text"
          value={apiUrl}
          onChange={(e) => setApiUrl(e.target.value)}
          placeholder="API URL"
          className="bg-slate-800 text-slate-200 px-2 py-1 rounded text-sm w-56 border border-slate-700"
        />
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="API key (optional)"
          className="bg-slate-800 text-slate-200 px-2 py-1 rounded text-sm w-40 border border-slate-700"
        />
      </header>

      {/* Tab content */}
      <main className="flex-1 overflow-auto p-6">
        {tab === 'chat' && <ChatPanel apiUrl={apiUrl} apiKey={apiKey} />}
        {tab === 'models' && <ModelPanel apiUrl={apiUrl} apiKey={apiKey} />}
        {tab === 'monitor' && <MonitorPanel apiUrl={apiUrl} apiKey={apiKey} />}
      </main>

      {/* Health bar */}
      <HealthBar health={health} error={healthError} />
    </div>
  )
}
