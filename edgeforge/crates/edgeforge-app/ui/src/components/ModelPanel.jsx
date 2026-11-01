import { useState, useEffect } from 'react'

/**
 * Model library panel.
 *
 * Lists imported models (GET /v1/models) and provides an import field
 * for adding new models by server-side file path.
 */
export default function ModelPanel({ apiUrl, apiKey }) {
  const [models, setModels] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)
  const [importPath, setImportPath] = useState('')

  async function fetchModels() {
    setLoading(true)
    setError(null)
    try {
      const headers = {}
      if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`
      const resp = await fetch(`${apiUrl}/v1/models`, { headers })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const body = await resp.json()
      setModels(body.data || [])
    } catch (e) {
      setError(e.message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchModels()
  }, [apiUrl, apiKey])

  async function importModel() {
    if (!importPath) return
    try {
      const headers = { 'Content-Type': 'application/json' }
      if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`
      const resp = await fetch(`${apiUrl}/v1/models/import`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ path: importPath }),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      setImportPath('')
      fetchModels()
    } catch (e) {
      setError(e.message)
    }
  }

  return (
    <div className="max-w-3xl mx-auto flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-200">Models</h2>
        <button
          onClick={fetchModels}
          className="text-sm text-cyan-400 hover:text-cyan-300"
        >
          Refresh
        </button>
      </div>

      {/* Import */}
      <div className="flex gap-2">
        <input
          type="text"
          value={importPath}
          onChange={(e) => setImportPath(e.target.value)}
          placeholder="/path/to/model.gguf (server-side)"
          className="flex-1 bg-slate-800 text-slate-200 px-3 py-2 rounded border border-slate-700"
        />
        <button
          onClick={importModel}
          disabled={!importPath}
          className="bg-cyan-600 hover:bg-cyan-700 disabled:bg-slate-700 text-white px-4 py-2 rounded"
        >
          Import
        </button>
      </div>

      {error && (
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-3 py-2 rounded text-sm">
          Error: {error}
        </div>
      )}

      {loading ? (
        <div className="text-slate-400">Loading models...</div>
      ) : models.length === 0 ? (
        <div className="text-slate-400 italic">No models imported yet.</div>
      ) : (
        <div className="flex flex-col gap-2">
          {models.map((m) => (
            <div
              key={m.id}
              className="bg-slate-800 border border-slate-700 rounded px-4 py-3 flex items-center justify-between"
            >
              <div>
                <div className="text-slate-200 font-mono text-sm">{m.id}</div>
                <div className="text-slate-500 text-xs">
                  owned by {m.owned_by}
                </div>
              </div>
              <span className="text-xs text-slate-500">
                {new Date((m.created || 0) * 1000).toLocaleDateString()}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
