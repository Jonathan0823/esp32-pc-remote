import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'

export default function Logs() {
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Logs</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">System logs coming soon.</p>
      </CardContent>
    </Card>
  )
}
