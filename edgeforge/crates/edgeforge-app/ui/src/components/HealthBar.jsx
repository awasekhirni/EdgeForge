/**
 * Health bar shown at the bottom of the app.
 * Shows green when healthy, red when the server is unreachable.
 */
export default function HealthBar({ health, error }) {
  const ok = health && health.status === 'ok'
  const color = ok ? 'bg-green-600' : 'bg-red-600'
  const message = ok
    ? `Server: ${health.status} (v${health.version})`
    : error
      ? `Server unreachable: ${error}`
      : 'Connecting...'

  return (
    <footer className={`${color} text-white px-4 py-2 text-sm flex items-center gap-2`}>
      <span className={`inline-block w-2 h-2 rounded-full ${ok ? 'bg-green-300' : 'bg-red-300'} ${ok ? '' : 'animate-pulse'}`} />
      {message}
    </footer>
  )
}
