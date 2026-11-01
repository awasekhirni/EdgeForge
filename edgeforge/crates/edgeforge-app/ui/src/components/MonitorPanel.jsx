import { useState, useEffect } from 'react'

/**
 * Monitoring panel.
 *
 * Displays SSD temperature, throughput, and health. Currently uses
 * synthetic data (the API server doesn't have a /monitor endpoint yet),
 * but the structure is ready for real monitoring data.
 */
export default function MonitorPanel({ apiUrl, apiKey }) {
  const [data, setData] = useState(null)
  const [error, setError] = useState(null)

  useEffect(() => {
    async function fetchMonitor() {
      // The API server doesn't have a /monitor endpoint yet, so we just
      // display a static dashboard. In the future, this would poll
      // /v1/monitor or read from the local thermal monitor.
      setData({
        ssd_temp_c: 45.0,
        tok_per_sec: 0.0,
        ssd_health: { percent_used: 10.0 },
        api_url: apiUrl,
      })
      setError(null)
    }
    fetchMonitor()
    const interval = setInterval(fetchMonitor, 5000)
    return () => clearInterval(interval)
  }, [apiUrl, apiKey])

  if (error) {
    return (
      <div className="max-w-3xl mx-auto">
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-3 py-2 rounded text-sm">
          Error: {error}
        </div>
      </div>
    )
  }

  if (!data) {
    return <div className="text-slate-400">Loading monitor data...</div>
  }

  const tempColor =
    data.ssd_temp_c < 60 ? 'text-green-400' :
    data.ssd_temp_c < 70 ? 'text-yellow-400' :
    'text-red-400'

  return (
    <div className="max-w-3xl mx-auto grid grid-cols-2 gap-4">
      <h2 className="col-span-2 text-lg font-semibold text-slate-200">Monitoring</h2>

      {/* SSD Temperature */}
      <div className="bg-slate-800 border border-slate-700 rounded p-4">
        <div className="text-slate-400 text-sm mb-1">SSD Temperature</div>
        <div className={`text-3xl font-bold ${tempColor}`}>
          {data.ssd_temp_c.toFixed(1)}°C
        </div>
        <div className="text-slate-500 text-xs mt-1">
          {data.ssd_temp_c < 60 ? 'Normal' :
           data.ssd_temp_c < 70 ? 'Throttled' :
           'Critical'}
        </div>
      </div>

      {/* Throughput */}
      <div className="bg-slate-800 border border-slate-700 rounded p-4">
        <div className="text-slate-400 text-sm mb-1">Throughput</div>
        <div className="text-3xl font-bold text-cyan-400">
          {data.tok_per_sec.toFixed(1)}
        </div>
        <div className="text-slate-500 text-xs mt-1">tokens/sec</div>
      </div>

      {/* SSD Health */}
      <div className="bg-slate-800 border border-slate-700 rounded p-4">
        <div className="text-slate-400 text-sm mb-1">SSD Health</div>
        <div className="text-3xl font-bold text-green-400">
          {(100 - data.ssd_health.percent_used).toFixed(1)}%
        </div>
        <div className="text-slate-500 text-xs mt-1">
          {data.ssd_health.percent_used.toFixed(1)}% used
        </div>
      </div>

      {/* API URL */}
      <div className="bg-slate-800 border border-slate-700 rounded p-4">
        <div className="text-slate-400 text-sm mb-1">API Server</div>
        <div className="text-sm font-mono text-slate-300 break-all">
          {data.api_url}
        </div>
      </div>
    </div>
  )
}
