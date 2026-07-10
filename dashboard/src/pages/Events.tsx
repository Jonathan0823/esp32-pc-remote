import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'

export default function Events() {
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Events</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-muted-foreground text-xs">Event log coming soon.</p>
      </CardContent>
    </Card>
  )
}
