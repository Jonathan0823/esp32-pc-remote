import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'

export default function Activity() {
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Activity</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-muted-foreground text-xs">Activity feed coming soon.</p>
      </CardContent>
    </Card>
  )
}
