import { useEffect, useState, memo } from 'react'
import { formatAgo, formatDuration } from '@/lib/helpers'

interface LiveTimeProps {
  refTimestamp: number
  uptimeBaseSeconds?: number
}

function LiveTime({ refTimestamp, uptimeBaseSeconds }: Readonly<LiveTimeProps>) {
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [])

  if (uptimeBaseSeconds !== undefined) {
    const elapsed = uptimeBaseSeconds + (now - refTimestamp) / 1000
    return <>{formatDuration(elapsed)}</>
  }

  return <>{formatAgo(refTimestamp)}</>
}

export default memo(LiveTime)
