import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'

export default function Events() {
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Events</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">Event log coming soon.</p>
      </CardContent>
    </Card>
  )
}
