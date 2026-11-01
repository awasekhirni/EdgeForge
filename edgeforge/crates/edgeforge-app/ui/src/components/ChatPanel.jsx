import { useState } from 'react'

/**
 * Chat completion panel.
 *
 * Sends POST /v1/chat/completions with the user's prompt and displays
 * the generated response.
 */
export default function ChatPanel({ apiUrl, apiKey }) {
  const [model, setModel] = useState('')
  const [prompt, setPrompt] = useState('')
  const [response, setResponse] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState(null)

  async function sendChat() {
    if (!model || !prompt) return
    setLoading(true)
    setError(null)
    setResponse('')
    try {
      const headers = { 'Content-Type': 'application/json' }
      if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`
      const resp = await fetch(`${apiUrl}/v1/chat/completions`, {
        method: 'POST',
        headers,
        body: JSON.stringify({
          model,
          messages: [{ role: 'user', content: prompt }],
          max_tokens: 512,
          temperature: 0.7,
          stream: false,
        }),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const body = await resp.json()
      setResponse(body.choices?.[0]?.message?.content || '(empty response)')
    } catch (e) {
      setError(e.message)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="max-w-3xl mx-auto flex flex-col gap-4">
      <h2 className="text-lg font-semibold text-slate-200">Chat Completion</h2>

      <div>
        <label className="block text-sm text-slate-400 mb-1">Model ID</label>
        <input
          type="text"
          value={model}
          onChange={(e) => setModel(e.target.value)}
          placeholder="e.g. 01J9XXXXXXXXXXXXXXXXXXX"
          className="w-full bg-slate-800 text-slate-200 px-3 py-2 rounded border border-slate-700"
        />
      </div>

      <div>
        <label className="block text-sm text-slate-400 mb-1">Prompt</label>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Type your message here..."
          rows={4}
          className="w-full bg-slate-800 text-slate-200 px-3 py-2 rounded border border-slate-700 resize-y"
        />
      </div>

      <button
        onClick={sendChat}
        disabled={loading || !model || !prompt}
        className="bg-cyan-600 hover:bg-cyan-700 disabled:bg-slate-700 disabled:text-slate-500 text-white px-4 py-2 rounded font-medium"
      >
        {loading ? 'Generating...' : 'Send'}
      </button>

      {error && (
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-3 py-2 rounded text-sm">
          Error: {error}
        </div>
      )}

      {response && (
        <div>
          <label className="block text-sm text-slate-400 mb-1">Response</label>
          <div className="bg-slate-800 text-slate-200 px-3 py-2 rounded border border-slate-700 whitespace-pre-wrap min-h-[100px]">
            {response}
          </div>
        </div>
      )}
    </div>
  )
}
